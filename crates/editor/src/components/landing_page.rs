use egui::{Color32, NumExt, RichText, Vec2};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;
use crate::ui::settings_screen::{RecentPlotEntry, WorkspaceEntry};

/// Action returned by the landing page
#[derive(Debug, Clone, PartialEq)]
pub enum LandingPageAction {
    /// No action
    None,
    /// Open a recent plot by metric name
    OpenPlot { metric_name: String, is_query: bool },
    /// Open a workspace
    OpenWorkspace { name: String },
    /// Open the fuzzy finder for metrics
    OpenFuzzyFinder,
    /// Open the query finder for saved queries
    OpenQueryFinder,
    /// Show info overlay
    ShowInfo,
    /// Show help
    ShowHelp,
    /// Open the command palette with :connect pre-filled
    OpenConnect,
}

/// The dashboard-nvim inspired landing page component
pub struct LandingPage {
    theme: AppTheme,
    /// Selected index in recent plots list (-1 means nothing selected)
    selected_plot_index: Option<usize>,
    /// Selected index in recent workspaces list
    selected_workspace_index: Option<usize>,
    /// Which list is focused (true = plots, false = workspaces)
    plots_focused: bool,
    /// Whether a shortcut button is focused
    shortcut_focused: Option<usize>,
}

impl Default for LandingPage {
    fn default() -> Self {
        Self::new()
    }
}

impl LandingPage {
    pub fn new() -> Self {
        Self {
            theme: AppTheme::default(),
            selected_plot_index: None,
            selected_workspace_index: None,
            plots_focused: true,
            shortcut_focused: None,
        }
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Show the landing page UI
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        recent_plots: &[RecentPlotEntry],
        recent_workspaces: &[WorkspaceEntry],
    ) -> LandingPageAction {
        // Handle keyboard navigation
        let action = self.handle_keyboard(ctx, recent_plots, recent_workspaces);
        if action != LandingPageAction::None {
            return action;
        }
        let mut action = LandingPageAction::None;

        let text_col = text_color(self.theme);
        let accent_color = self.accent_color();
        let muted_color = text_col.gamma_multiply(0.5);

        egui::Frame {
            inner_margin: egui::Margin::same(20),
            ..Default::default()
        }
        .show(ui, |ui| {
            // Center content with max width
            const MAX_WIDTH: f32 = 800.0;
            let centering_margin = ((ui.available_width() - MAX_WIDTH) / 2.0).at_least(0.0);
            let max_rect = ui.max_rect().expand2(-centering_margin * egui::Vec2::X);
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(max_rect));

            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(&mut child_ui, |ui| {
                    ui.vertical_centered(|ui| {
                        // Add some top padding
                        ui.add_space(40.0);

                        // === HEADER SECTION ===
                        self.show_header(ui, text_col, muted_color);

                        ui.add_space(40.0);

                        // === RECENT ITEMS SECTION (Two columns) ===
                        action = self.show_recent_sections(
                            ui,
                            text_col,
                            accent_color,
                            muted_color,
                            recent_plots,
                            recent_workspaces,
                        );

                        ui.add_space(30.0);

                        // === SHORTCUTS SECTION ===
                        if action == LandingPageAction::None {
                            action = self.show_shortcuts(ui, text_col, accent_color, muted_color);
                        }

                        ui.add_space(40.0);

                        // === FOOTER ===
                        self.show_footer(ui, muted_color);
                    });
                });
        });

        action
    }

    /// Show the header with logo and title
    fn show_header(&self, ui: &mut egui::Ui, _text_col: Color32, muted_color: Color32) {
        // Logo
        let logo = egui::Image::new(egui::include_image!("../../assets/logo.png"));
        ui.add(logo.max_width(180.0).max_height(180.0));

        ui.add_space(16.0);

        // App name in Enya's signature golden yellow
        let enya_yellow = self.accent_color();
        ui.heading(RichText::new("ENYA").strong().size(36.0).color(enya_yellow));

        ui.add_space(8.0);

        // Tagline
        ui.label(
            RichText::new("Your trusted companion for building and running data systems")
                .size(14.0)
                .color(muted_color),
        );
    }

    /// Show the two-column recent items section
    fn show_recent_sections(
        &mut self,
        ui: &mut egui::Ui,
        text_col: Color32,
        accent_color: Color32,
        muted_color: Color32,
        recent_plots: &[RecentPlotEntry],
        recent_workspaces: &[WorkspaceEntry],
    ) -> LandingPageAction {
        let mut action = LandingPageAction::None;

        // Calculate column widths
        let available_width = ui.available_width();
        let column_width = (available_width - 40.0) / 2.0; // 40px gap between columns
        let column_width = column_width.at_most(350.0);

        ui.horizontal(|ui| {
            // Center the columns if there's extra space
            let total_columns_width = column_width * 2.0 + 40.0;
            let offset = ((available_width - total_columns_width) / 2.0).at_least(0.0);
            ui.add_space(offset);

            // Left column: Recent Plots
            ui.vertical(|ui| {
                ui.set_width(column_width);
                action = self.show_recent_plots_column(
                    ui,
                    text_col,
                    accent_color,
                    muted_color,
                    recent_plots,
                );
            });

            ui.add_space(40.0);

            // Right column: Recent Workspaces
            ui.vertical(|ui| {
                ui.set_width(column_width);
                if action == LandingPageAction::None {
                    action = self.show_recent_workspaces_column(
                        ui,
                        text_col,
                        accent_color,
                        muted_color,
                        recent_workspaces,
                    );
                }
            });
        });

        action
    }

    /// Show the recent plots column
    fn show_recent_plots_column(
        &mut self,
        ui: &mut egui::Ui,
        text_col: Color32,
        accent_color: Color32,
        muted_color: Color32,
        recent_plots: &[RecentPlotEntry],
    ) -> LandingPageAction {
        let mut action = LandingPageAction::None;

        // Section header (white/light text color, not accent)
        let header_text = format!("{}  Recent Plots", semantic_icons::action::CHART);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(header_text)
                    .strong()
                    .size(semantic_icons::SIZE_HEADER)
                    .color(text_col),
            );
        });

        ui.add_space(8.0);

        // Divider
        ui.separator();
        ui.add_space(4.0);

        if recent_plots.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(semantic_icons::empty::NO_PLOTS)
                        .size(semantic_icons::SIZE_ITEM)
                        .color(muted_color),
                );
                ui.label(
                    RichText::new("No recent plots")
                        .size(13.0)
                        .color(muted_color)
                        .italics(),
                );
            });
        } else {
            for (idx, plot) in recent_plots.iter().enumerate().take(8) {
                let is_selected = self.plots_focused && self.selected_plot_index == Some(idx);

                let response = self.show_list_item(
                    ui,
                    &plot.name,
                    if plot.is_query {
                        semantic_icons::file::CODE
                    } else {
                        semantic_icons::action::CHART
                    },
                    text_col,
                    accent_color,
                    is_selected,
                    Some(format!("{}", idx + 1)), // Shortcut hint: 1-8
                );

                if response.clicked() {
                    action = LandingPageAction::OpenPlot {
                        metric_name: plot.metric_name.clone(),
                        is_query: plot.is_query,
                    };
                }

                if response.hovered() {
                    self.selected_plot_index = Some(idx);
                    self.plots_focused = true;
                }
            }
        }

        action
    }

    /// Show the recent workspaces column
    fn show_recent_workspaces_column(
        &mut self,
        ui: &mut egui::Ui,
        text_col: Color32,
        accent_color: Color32,
        muted_color: Color32,
        recent_workspaces: &[WorkspaceEntry],
    ) -> LandingPageAction {
        let mut action = LandingPageAction::None;

        // Section header (white/light text color, not accent)
        let header_text = format!("{}  Recent Workspaces", semantic_icons::file::FOLDER_OPEN);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(header_text)
                    .strong()
                    .size(semantic_icons::SIZE_HEADER)
                    .color(text_col),
            );
        });

        ui.add_space(8.0);

        // Divider
        ui.separator();
        ui.add_space(4.0);

        if recent_workspaces.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(semantic_icons::empty::NO_WORKSPACES)
                        .size(semantic_icons::SIZE_ITEM)
                        .color(muted_color),
                );
                ui.label(
                    RichText::new("No recent workspaces")
                        .size(13.0)
                        .color(muted_color)
                        .italics(),
                );
            });
        } else {
            for (idx, workspace) in recent_workspaces.iter().enumerate().take(8) {
                let is_selected = !self.plots_focused && self.selected_workspace_index == Some(idx);

                let response = self.show_list_item(
                    ui,
                    &workspace.name,
                    semantic_icons::file::FOLDER,
                    text_col,
                    accent_color,
                    is_selected,
                    None, // No shortcut hint for workspaces
                );

                if response.clicked() {
                    action = LandingPageAction::OpenWorkspace {
                        name: workspace.name.clone(),
                    };
                }

                if response.hovered() {
                    self.selected_workspace_index = Some(idx);
                    self.plots_focused = false;
                }
            }
        }

        action
    }

    /// Show a single list item (plot or workspace)
    #[allow(clippy::too_many_arguments)]
    fn show_list_item(
        &self,
        ui: &mut egui::Ui,
        label: &str,
        icon: &str,
        text_col: Color32,
        accent_color: Color32,
        is_selected: bool,
        shortcut_hint: Option<String>,
    ) -> egui::Response {
        let item_height = 28.0;
        let available_width = ui.available_width();

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(available_width, item_height),
            egui::Sense::click(),
        );

        // Background on hover/select
        if is_selected || response.hovered() {
            let bg_color = if is_selected {
                accent_color.gamma_multiply(0.15)
            } else {
                text_col.gamma_multiply(0.05)
            };
            ui.painter().rect_filled(rect, 4.0, bg_color);
        }

        // Icon
        let icon_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(8.0, (item_height - 16.0) / 2.0),
            egui::vec2(16.0, 16.0),
        );
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(14.0),
            if is_selected {
                accent_color
            } else {
                text_col.gamma_multiply(0.7)
            },
        );

        // Label
        let label_color = if is_selected { accent_color } else { text_col };
        ui.painter().text(
            egui::pos2(rect.min.x + 32.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(13.0),
            label_color,
        );

        // Shortcut hint on the right
        if let Some(hint) = shortcut_hint {
            ui.painter().text(
                egui::pos2(rect.max.x - 12.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                hint,
                egui::FontId::proportional(11.0),
                text_col.gamma_multiply(0.4),
            );
        }

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Show the action shortcuts bar
    fn show_shortcuts(
        &mut self,
        ui: &mut egui::Ui,
        text_col: Color32,
        accent_color: Color32,
        muted_color: Color32,
    ) -> LandingPageAction {
        let mut action = LandingPageAction::None;
        let _ = muted_color; // unused now

        // Shortcuts row
        ui.horizontal(|ui| {
            // Center the shortcuts
            let shortcut_width = 100.0;
            let num_shortcuts = 5;
            let gap = 12.0;
            let total_width =
                shortcut_width * num_shortcuts as f32 + gap * (num_shortcuts - 1) as f32;
            let offset = ((ui.available_width() - total_width) / 2.0).at_least(0.0);
            ui.add_space(offset);

            // Metrics (m)
            if self
                .show_shortcut_button(
                    ui,
                    semantic_icons::action::SEARCH,
                    "Metrics",
                    "m",
                    text_col,
                    accent_color,
                    self.shortcut_focused == Some(0),
                    shortcut_width,
                )
                .clicked()
            {
                action = LandingPageAction::OpenFuzzyFinder;
            }

            ui.add_space(gap);

            // Queries (q)
            if self
                .show_shortcut_button(
                    ui,
                    semantic_icons::file::CODE,
                    "Queries",
                    "q",
                    text_col,
                    accent_color,
                    self.shortcut_focused == Some(1),
                    shortcut_width,
                )
                .clicked()
            {
                action = LandingPageAction::OpenQueryFinder;
            }

            ui.add_space(gap);

            // Connect (c)
            if self
                .show_shortcut_button(
                    ui,
                    semantic_icons::status::PLUG,
                    "Connect",
                    "c",
                    text_col,
                    accent_color,
                    self.shortcut_focused == Some(2),
                    shortcut_width,
                )
                .clicked()
            {
                action = LandingPageAction::OpenConnect;
            }

            ui.add_space(gap);

            // Info (i)
            if self
                .show_shortcut_button(
                    ui,
                    semantic_icons::status::INFO,
                    "Info",
                    "i",
                    text_col,
                    accent_color,
                    self.shortcut_focused == Some(3),
                    shortcut_width,
                )
                .clicked()
            {
                action = LandingPageAction::ShowInfo;
            }

            ui.add_space(gap);

            // Help (?)
            if self
                .show_shortcut_button(
                    ui,
                    semantic_icons::status::QUESTION,
                    "Help",
                    "?",
                    text_col,
                    accent_color,
                    self.shortcut_focused == Some(4),
                    shortcut_width,
                )
                .clicked()
            {
                action = LandingPageAction::ShowHelp;
            }
        });

        action
    }

    /// Show a single shortcut button
    #[allow(clippy::too_many_arguments)]
    fn show_shortcut_button(
        &self,
        ui: &mut egui::Ui,
        icon: &str,
        label: &str,
        shortcut: &str,
        text_col: Color32,
        accent_color: Color32,
        is_focused: bool,
        width: f32,
    ) -> egui::Response {
        let height = 70.0;

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click());

        // Background
        let bg_color = if is_focused {
            accent_color.gamma_multiply(0.15)
        } else if response.hovered() {
            text_col.gamma_multiply(0.05)
        } else {
            Color32::TRANSPARENT
        };

        // Use white/gray border instead of accent color
        let stroke_color = if is_focused || response.hovered() {
            text_col.gamma_multiply(0.4)
        } else {
            text_col.gamma_multiply(0.1)
        };

        ui.painter().rect(
            rect,
            8.0,
            bg_color,
            egui::Stroke::new(1.0, stroke_color),
            egui::StrokeKind::Inside,
        );

        // Icon
        let icon_color = if is_focused || response.hovered() {
            accent_color
        } else {
            text_col.gamma_multiply(0.7)
        };
        ui.painter().text(
            rect.center_top() + egui::vec2(0.0, 20.0),
            egui::Align2::CENTER_CENTER,
            icon,
            egui::FontId::proportional(semantic_icons::SIZE_LARGE),
            icon_color,
        );

        // Label
        ui.painter().text(
            rect.center_top() + egui::vec2(0.0, 42.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(12.0),
            text_col,
        );

        // Shortcut key
        ui.painter().text(
            rect.center_top() + egui::vec2(0.0, 58.0),
            egui::Align2::CENTER_CENTER,
            shortcut,
            egui::FontId::proportional(13.0),
            text_col.gamma_multiply(0.6),
        );

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// Show the footer
    fn show_footer(&self, ui: &mut egui::Ui, muted_color: Color32) {
        ui.label(
            RichText::new(format!(
                "v{}  •  Developed by Meldrum Labs",
                env!("CARGO_PKG_VERSION")
            ))
            .size(11.0)
            .color(muted_color),
        );
    }

    /// Handle keyboard navigation
    fn handle_keyboard(
        &mut self,
        ctx: &egui::Context,
        recent_plots: &[RecentPlotEntry],
        recent_workspaces: &[WorkspaceEntry],
    ) -> LandingPageAction {
        // Don't handle keys if a text field has focus
        if ctx.memory(|mem| mem.focused().is_some()) {
            return LandingPageAction::None;
        }

        let mut action = LandingPageAction::None;

        ctx.input_mut(|input| {
            // Number keys 1-8 for quick plot selection
            for (idx, key) in [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
                egui::Key::Num8,
            ]
            .iter()
            .enumerate()
            {
                if input.consume_key(egui::Modifiers::NONE, *key) {
                    if let Some(plot) = recent_plots.get(idx) {
                        action = LandingPageAction::OpenPlot {
                            metric_name: plot.metric_name.clone(),
                            is_query: plot.is_query,
                        };
                        return;
                    }
                }
            }

            // m - Find metrics (fuzzy finder)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::M) {
                action = LandingPageAction::OpenFuzzyFinder;
                return;
            }

            // q - Open queries (query finder)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Q) {
                action = LandingPageAction::OpenQueryFinder;
                return;
            }

            // c - Connect
            if input.consume_key(egui::Modifiers::NONE, egui::Key::C) {
                action = LandingPageAction::OpenConnect;
                return;
            }

            // i - Info
            if input.consume_key(egui::Modifiers::NONE, egui::Key::I) {
                action = LandingPageAction::ShowInfo;
                return;
            }

            // ? - Help (check for '?' character in text input, or Shift+/)
            // First check raw text events for '?' which works across keyboard layouts
            let has_question_mark = input
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "?"));
            if has_question_mark || input.consume_key(egui::Modifiers::SHIFT, egui::Key::Slash) {
                // Consume the text event to prevent it from being handled elsewhere
                input
                    .events
                    .retain(|e| !matches!(e, egui::Event::Text(t) if t == "?"));
                action = LandingPageAction::ShowHelp;
                return;
            }

            // j/Down - Move down in list
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                if self.plots_focused {
                    let max_idx = recent_plots.len().saturating_sub(1).min(7);
                    self.selected_plot_index = Some(
                        self.selected_plot_index
                            .map(|i| (i + 1).min(max_idx))
                            .unwrap_or(0),
                    );
                } else {
                    let max_idx = recent_workspaces.len().saturating_sub(1).min(7);
                    self.selected_workspace_index = Some(
                        self.selected_workspace_index
                            .map(|i| (i + 1).min(max_idx))
                            .unwrap_or(0),
                    );
                }
                return;
            }

            // k/Up - Move up in list
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                if self.plots_focused {
                    self.selected_plot_index = Some(
                        self.selected_plot_index
                            .map(|i| i.saturating_sub(1))
                            .unwrap_or(0),
                    );
                } else {
                    self.selected_workspace_index = Some(
                        self.selected_workspace_index
                            .map(|i| i.saturating_sub(1))
                            .unwrap_or(0),
                    );
                }
                return;
            }

            // h/Left - Switch to plots list
            if input.consume_key(egui::Modifiers::NONE, egui::Key::H)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
            {
                self.plots_focused = true;
                return;
            }

            // l/Right - Switch to workspaces list
            if input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                self.plots_focused = false;
                return;
            }

            // Enter - Select current item
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                if self.plots_focused {
                    if let Some(idx) = self.selected_plot_index {
                        if let Some(plot) = recent_plots.get(idx) {
                            action = LandingPageAction::OpenPlot {
                                metric_name: plot.metric_name.clone(),
                                is_query: plot.is_query,
                            };
                        }
                    }
                } else if let Some(idx) = self.selected_workspace_index {
                    if let Some(workspace) = recent_workspaces.get(idx) {
                        action = LandingPageAction::OpenWorkspace {
                            name: workspace.name.clone(),
                        };
                    }
                }
            }
        });

        action
    }

    /// Get the accent color based on theme (Enya's signature golden/yellow)
    fn accent_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(180, 140, 20), // Golden yellow for light theme
            AppTheme::Dark => Color32::from_rgb(255, 200, 50),  // Bright gold for dark theme
        }
    }

    /// Get the highlight color for match highlighting (golden yellow)
    fn _highlight_color(&self) -> Color32 {
        match self.theme {
            AppTheme::Light => Color32::from_rgb(200, 150, 0),
            AppTheme::Dark => Color32::from_rgb(255, 200, 50),
        }
    }
}
