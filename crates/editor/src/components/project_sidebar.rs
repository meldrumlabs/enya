//! Project sidebar — persistent left panel for workspace navigation.
//!
//! Inspired by conductor.build / superset.sh sidebar patterns, adapted for
//! observability workspaces. Renders as `egui::SidePanel::left` at the app
//! level (before `CentralPanel`).
//!
//! Supports project grouping: workspaces can be organized under collapsible
//! project headers. Ungrouped workspaces appear after all project sections.
//!
//! Vim-friendly: `[` focuses the sidebar, `j`/`k` navigate, `Enter` loads
//! or toggles collapse, `Escape` returns focus to the workspace.

use egui::{Color32, RichText, Vec2};
use rustc_hash::FxHashSet;

use crate::ui::semantic_icons;
use crate::ui::settings_screen::AppSettings;
use crate::ui::theme::AppTheme;
use crate::ui::typography;
use crate::util::Instant;

/// A workspace entry displayed in the sidebar.
#[derive(Clone)]
pub struct SidebarWorkspaceItem {
    pub name: String,
    pub description: Option<String>,
    /// Unix timestamp of last access (0 if unknown).
    pub last_accessed: i64,
}

/// A flat navigation item for keyboard traversal.
#[derive(Clone, Debug)]
enum SidebarNavItem {
    ProjectHeader {
        name: String,
        workspace_count: usize,
        collapsed: bool,
    },
    Workspace {
        name: String,
        /// Extra left indent when inside a project.
        indented: bool,
    },
}

/// Result returned each frame from the sidebar.
pub enum ProjectSidebarResult {
    None,
    /// Load a workspace and unfocus the sidebar (click or Enter).
    LoadWorkspace(String),
    /// Load a workspace but keep sidebar focused (j/k navigation).
    PreviewWorkspace(String),
    /// Create an empty workspace (no wizard).
    CreateEmptyWorkspace,
    /// Create an empty workspace inside a specific project (no wizard).
    CreateEmptyWorkspaceInProject(String),
    /// Toggle a project section's collapsed state.
    ToggleProjectCollapse(String),
    /// Open the project creation wizard.
    CreateProject,
    OpenSettings,
    /// Archive (delete) a workspace.
    ArchiveWorkspace(String),
    /// Sidebar lost focus — return keyboard control to workspace
    Unfocused,
    /// User clicked the close button — hide the sidebar.
    Closed,
}

const SIDEBAR_WIDTH: f32 = 240.0;
const SCAN_COOLDOWN_SECS: f64 = 2.0;

pub struct ProjectSidebar {
    is_visible: bool,
    /// Whether the sidebar currently owns keyboard focus (j/k/Enter active).
    has_focus: bool,
    theme: AppTheme,
    workspaces: Vec<SidebarWorkspaceItem>,
    active_workspace: Option<String>,
    /// Flat navigation items (project headers + workspaces).
    nav_items: Vec<SidebarNavItem>,
    /// Keyboard selection index within `nav_items`.
    selected_index: usize,
    last_scan: Option<Instant>,
}

impl Default for ProjectSidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectSidebar {
    pub fn new() -> Self {
        Self {
            is_visible: true,
            has_focus: false,
            theme: AppTheme::default(),
            workspaces: Vec::new(),
            active_workspace: None,
            nav_items: Vec::new(),
            selected_index: 0,
            last_scan: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// Give keyboard focus to the sidebar (j/k navigation).
    pub fn focus(&mut self) {
        self.has_focus = true;
        // Pre-select the active workspace if any
        if let Some(ref active) = self.active_workspace {
            if let Some(idx) = self.nav_items.iter().position(
                |item| matches!(item, SidebarNavItem::Workspace { name, .. } if name == active),
            ) {
                self.selected_index = idx;
            }
        }
    }

    /// Remove keyboard focus from the sidebar.
    pub fn unfocus(&mut self) {
        self.has_focus = false;
    }

    pub fn toggle(&mut self) {
        self.is_visible = !self.is_visible;
        if !self.is_visible {
            self.has_focus = false;
        }
    }

    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    pub fn set_active_workspace(&mut self, name: Option<&str>) {
        self.active_workspace = name.map(|s| s.to_string());
    }

    /// Rebuild just the nav items from current workspaces + settings.
    /// Cheap operation — no filesystem scan. Use after toggling project
    /// collapse or creating/deleting projects.
    pub fn rebuild(&mut self, settings: &AppSettings) {
        self.rebuild_nav_items(settings);
    }

    /// Clear the scan cooldown so the next `refresh_workspaces` does a full rescan.
    pub fn force_rescan(&mut self) {
        self.last_scan = None;
    }

    /// Refresh the workspace list from settings + filesystem, then rebuild
    /// the flat `nav_items` list for rendering and keyboard navigation.
    pub fn refresh_workspaces(&mut self, settings: &AppSettings) {
        let now = Instant::now();
        let should_scan = self
            .last_scan
            .map(|t| now.duration_since(t).as_secs_f64() > SCAN_COOLDOWN_SECS)
            .unwrap_or(true);

        if !should_scan {
            return;
        }
        self.last_scan = Some(now);

        let mut items: Vec<SidebarWorkspaceItem> = Vec::new();
        let mut seen = FxHashSet::default();

        // Recent workspaces first (most recently used order)
        for entry in &settings.recent_workspaces {
            if seen.insert(entry.name.clone()) {
                items.push(SidebarWorkspaceItem {
                    name: entry.name.clone(),
                    description: if entry.description.is_empty() {
                        None
                    } else {
                        Some(entry.description.clone())
                    },
                    last_accessed: entry.timestamp,
                });
            }
        }

        // Filesystem workspaces not already in the recent list
        #[cfg(not(target_arch = "wasm32"))]
        {
            let available = enya_config::list_workspaces();
            for (name, description) in available {
                if seen.insert(name.clone()) {
                    items.push(SidebarWorkspaceItem {
                        name,
                        description,
                        last_accessed: 0,
                    });
                }
            }
        }

        self.workspaces = items;

        // Rebuild nav items
        self.rebuild_nav_items(settings);
    }

    /// Build the flat `nav_items` list from projects + workspaces.
    fn rebuild_nav_items(&mut self, settings: &AppSettings) {
        let mut nav = Vec::new();
        let mut grouped = FxHashSet::default();

        // All known workspace names (for checking existence)
        let known: FxHashSet<&str> = self.workspaces.iter().map(|w| w.name.as_str()).collect();

        for project in &settings.projects {
            // On native, hide the Tutorial project unless a tutorial workspace is active.
            // On WASM, always show it since tutorials are the primary content.
            #[cfg(not(target_arch = "wasm32"))]
            if project.name == "Tutorial" {
                let tutorial_active = self
                    .active_workspace
                    .as_ref()
                    .is_some_and(|active| project.workspace_names.contains(active));
                if !tutorial_active {
                    for ws_name in &project.workspace_names {
                        grouped.insert(ws_name.clone());
                    }
                    continue;
                }
            }

            // Only count workspaces that actually exist
            let existing: Vec<&str> = project
                .workspace_names
                .iter()
                .filter(|w| known.contains(w.as_str()))
                .map(|w| w.as_str())
                .collect();

            nav.push(SidebarNavItem::ProjectHeader {
                name: project.name.clone(),
                workspace_count: existing.len(),
                collapsed: project.collapsed,
            });

            if !project.collapsed {
                for ws_name in &existing {
                    grouped.insert(ws_name.to_string());
                    nav.push(SidebarNavItem::Workspace {
                        name: ws_name.to_string(),
                        indented: true,
                    });
                }
            } else {
                // Still mark them as grouped even when collapsed
                for ws_name in &existing {
                    grouped.insert(ws_name.to_string());
                }
            }
        }

        // Ungrouped workspaces (native only — on WASM only project-grouped tutorials are shown)
        #[cfg(not(target_arch = "wasm32"))]
        for ws in &self.workspaces {
            if !grouped.contains(&ws.name) {
                nav.push(SidebarNavItem::Workspace {
                    name: ws.name.clone(),
                    indented: false,
                });
            }
        }

        self.nav_items = nav;

        // Clamp selection
        let count = self.nav_items.len();
        if count > 0 && self.selected_index >= count {
            self.selected_index = count - 1;
        }
    }

    /// Show the sidebar panel inside the given `Ui` (must be called inside `CentralPanel`
    /// so it matches the agent panel's height and doesn't touch titlebar/statusbar).
    ///
    /// Stores the sidebar width in ctx temp data so overlays can center within the
    /// content area via [`crate::util::overlay_content_rect`].
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> ProjectSidebarResult {
        let mut result = ProjectSidebarResult::None;

        let bg = self.theme.bg_surface();
        // Accent border when keyboard-focused; subtle otherwise (matches agent panel)
        let (border_color, border_width) = if self.has_focus {
            (self.theme.accent_primary(), 2.0)
        } else {
            (self.theme.border_subtle(), 1.0)
        };

        // Total width includes an 8px gap on the right for visual separation
        let gap = 8.0;

        // Store sidebar width so overlays can offset their centering
        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("sidebar_width"), SIDEBAR_WIDTH + gap);
        });

        egui::SidePanel::left("project_sidebar")
            .exact_width(SIDEBAR_WIDTH + gap)
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                // Paint the sidebar background and border inside the allocated area,
                // leaving the rightmost `gap` pixels transparent (shows parent bg).
                let full_rect = ui.max_rect();
                let sidebar_rect = full_rect.with_max_x(full_rect.max.x - gap);

                // Background fill
                ui.painter().rect_filled(sidebar_rect, 0.0, bg);
                // Border
                ui.painter().rect_stroke(
                    sidebar_rect,
                    0.0,
                    egui::Stroke::new(border_width, border_color),
                    egui::StrokeKind::Inside,
                );

                // Constrain the child ui to the sidebar area (not the gap)
                let mut child_rect = sidebar_rect;
                child_rect.min.x += border_width;
                child_rect.min.y += border_width;
                child_rect.max.x -= border_width;
                child_rect.max.y -= border_width;
                let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(child_rect));
                result = self.render_content(&mut child_ui, ctx);
            });

        result
    }

    fn render_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> ProjectSidebarResult {
        let mut result = ProjectSidebarResult::None;

        let accent = self.theme.accent_primary();
        let text_primary = self.theme.text_primary();
        let text_secondary = self.theme.text_secondary();
        let text_tertiary = self.theme.text_tertiary();

        let nav_count = self.nav_items.len();

        // Clamp selection to valid range
        if nav_count > 0 && self.selected_index >= nav_count {
            self.selected_index = nav_count - 1;
        }

        // ── Keyboard handling (only when sidebar has focus) ──────────
        if self.has_focus && !ctx.wants_keyboard_input() {
            let kb_result = self.handle_keyboard(ctx);
            if !matches!(kb_result, ProjectSidebarResult::None) {
                result = kb_result;
            }
        }

        // ── Header with close button ─────────────────────────────────
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.label(
                RichText::new("Projects")
                    .color(text_tertiary)
                    .font(typography::proportional(typography::XS)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let close_btn = ui.add(
                    egui::Button::new(
                        RichText::new(semantic_icons::action::CLOSE)
                            .size(14.0)
                            .color(text_tertiary),
                    )
                    .frame(false),
                );
                if close_btn.hovered() {
                    let rect = close_btn.rect.expand(4.0);
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(4),
                        self.theme.bg_elevated(),
                    );
                }
                if close_btn.on_hover_text("Close sidebar (Space+b)").clicked() {
                    result = ProjectSidebarResult::Closed;
                }
            });
        });
        ui.add_space(4.0);

        // ── Nav items list ──────────────────────────────────────────

        // Clone nav_items to avoid borrow issues during rendering
        let nav_snapshot: Vec<SidebarNavItem> = self.nav_items.clone();

        // Reserve space for the footer so the scroll area doesn't consume it
        let footer_height = 32.0;
        let scroll_max = (ui.available_height() - footer_height).max(0.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(scroll_max)
            .show(ui, |ui| {
                if nav_snapshot.is_empty() {
                    // Empty state — no workspaces at all
                    ui.add_space(32.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(semantic_icons::empty::NO_WORKSPACES)
                                .color(text_tertiary.gamma_multiply(0.5))
                                .font(egui::FontId::proportional(28.0)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("No projects yet")
                                .color(text_tertiary)
                                .font(typography::proportional(typography::SM)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Create one to get started")
                                .color(text_tertiary.gamma_multiply(0.6))
                                .font(typography::proportional(typography::XS)),
                        );
                    });
                } else {
                    for (idx, nav_item) in nav_snapshot.iter().enumerate() {
                        let is_selected = self.has_focus && idx == self.selected_index;

                        match nav_item {
                            SidebarNavItem::ProjectHeader {
                                name,
                                workspace_count,
                                collapsed,
                            } => {
                                if let Some(action) = self.render_project_header(
                                    ui,
                                    name,
                                    *workspace_count,
                                    *collapsed,
                                    is_selected,
                                    accent,
                                    text_primary,
                                    text_secondary,
                                    text_tertiary,
                                ) {
                                    result = action;
                                }
                            }
                            SidebarNavItem::Workspace { name, indented } => {
                                let ws = self.workspaces.iter().find(|w| w.name == *name);
                                let is_active =
                                    self.active_workspace.as_deref().is_some_and(|a| a == name);

                                if let Some(ws) = ws {
                                    if let Some(action) = self.render_workspace_row(
                                        ui,
                                        ws,
                                        is_active,
                                        is_selected,
                                        *indented,
                                        accent,
                                        text_primary,
                                        text_secondary,
                                        text_tertiary,
                                    ) {
                                        result = action;
                                    }
                                }
                            }
                        }
                    }
                }
            });

        // ── Footer ──────────────────────────────────────────────────
        self.render_footer(ui, &mut result, text_tertiary);

        result
    }

    /// Handle vim-style keyboard navigation when the sidebar has focus.
    ///
    /// j/k immediately loads the workspace under the cursor (no Enter needed).
    /// Enter on a project header toggles collapse.
    fn handle_keyboard(&mut self, ctx: &egui::Context) -> ProjectSidebarResult {
        let count = self.nav_items.len();
        let mut result = ProjectSidebarResult::None;
        let mut selection_moved = false;

        ctx.input_mut(|input| {
            // x — close (hide) sidebar
            if input.consume_key(egui::Modifiers::NONE, egui::Key::X) {
                self.has_focus = false;
                result = ProjectSidebarResult::Closed;
                return;
            }

            // Escape or l — unfocus sidebar, return to workspace
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::L)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
            {
                self.has_focus = false;
                result = ProjectSidebarResult::Unfocused;
                return;
            }

            // j / ArrowDown — move selection down
            if input.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                if count > 0 {
                    self.selected_index = (self.selected_index + 1) % count;
                    selection_moved = true;
                }
                return;
            }

            // k / ArrowUp — move selection up
            if input.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                if count > 0 {
                    self.selected_index = if self.selected_index == 0 {
                        count - 1
                    } else {
                        self.selected_index - 1
                    };
                    selection_moved = true;
                }
                return;
            }

            // g — jump to top
            if input.consume_key(egui::Modifiers::NONE, egui::Key::G) {
                self.selected_index = 0;
                selection_moved = true;
                return;
            }

            // G (Shift+g) — jump to bottom
            if input.consume_key(egui::Modifiers::SHIFT, egui::Key::G) {
                if count > 0 {
                    self.selected_index = count - 1;
                    selection_moved = true;
                }
                return;
            }

            // Enter — toggle project collapse (workspaces load on navigate)
            if input.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                if let Some(item) = self.nav_items.get(self.selected_index) {
                    match item {
                        SidebarNavItem::ProjectHeader { name, .. } => {
                            result = ProjectSidebarResult::ToggleProjectCollapse(name.clone());
                        }
                        SidebarNavItem::Workspace { name, .. } => {
                            result = ProjectSidebarResult::LoadWorkspace(name.clone());
                        }
                    }
                }
            }
        });

        // When selection moves to a workspace, preview it (load but keep focus)
        if selection_moved {
            if let Some(SidebarNavItem::Workspace { name, .. }) =
                self.nav_items.get(self.selected_index)
            {
                result = ProjectSidebarResult::PreviewWorkspace(name.clone());
            }
        }

        result
    }

    /// Render a project header row.
    #[allow(clippy::too_many_arguments)]
    fn render_project_header(
        &self,
        ui: &mut egui::Ui,
        name: &str,
        workspace_count: usize,
        collapsed: bool,
        is_selected: bool,
        accent: Color32,
        text_primary: Color32,
        _text_secondary: Color32,
        text_tertiary: Color32,
    ) -> Option<ProjectSidebarResult> {
        let row_height = 32.0;
        let avail_width = ui.available_width();

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(avail_width, row_height), egui::Sense::click());

        // Use rect_contains_pointer for visual hover so sub-interactions
        // (like the "+" button) don't cause the row to flicker.
        let hovered = ui.rect_contains_pointer(rect);

        // ── Background ──
        if is_selected {
            let sel_bg = accent.gamma_multiply(0.06);
            ui.painter().rect_filled(rect, 0.0, sel_bg);
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(2.0, row_height));
            ui.painter()
                .rect_filled(bar, 0.0, accent.gamma_multiply(0.5));
        } else if hovered {
            let hover_bg = text_primary.gamma_multiply(0.03);
            ui.painter().rect_filled(rect, 0.0, hover_bg);
        }

        // ── Collapse indicator ──
        let indicator_x = rect.min.x + 10.0;
        let indicator_icon = if collapsed {
            semantic_icons::nav::COLLAPSE // ▶ (points right when collapsed)
        } else {
            semantic_icons::nav::EXPAND // ▼ (points down when expanded)
        };
        let indicator_color = if is_selected {
            accent
        } else {
            text_tertiary.gamma_multiply(0.7)
        };
        ui.painter().text(
            egui::pos2(indicator_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            indicator_icon,
            egui::FontId::proportional(10.0),
            indicator_color,
        );

        // ── Project name ──
        let name_x = indicator_x + 14.0;
        let name_color = if is_selected {
            text_primary
        } else {
            text_primary.gamma_multiply(0.85)
        };
        ui.painter().text(
            egui::pos2(name_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            name,
            typography::proportional(typography::SM),
            name_color,
        );

        // ── Right side: workspace count badge + "+" button on hover ──
        let right_edge = rect.max.x - 10.0;

        // "+" button (visible on hover or selected, native only — hidden on WASM demo)
        #[cfg(not(target_arch = "wasm32"))]
        if hovered || is_selected {
            let plus_x = right_edge;
            let plus_rect = egui::Rect::from_center_size(
                egui::pos2(plus_x - 6.0, rect.center().y),
                Vec2::new(18.0, 18.0),
            );
            let plus_response = ui.interact(
                plus_rect,
                ui.id().with(("project_add", name)),
                egui::Sense::click(),
            );
            ui.painter().text(
                plus_rect.center(),
                egui::Align2::CENTER_CENTER,
                semantic_icons::action::ADD,
                egui::FontId::proportional(12.0),
                accent,
            );
            if plus_response.clicked() {
                return Some(ProjectSidebarResult::CreateEmptyWorkspaceInProject(
                    name.to_string(),
                ));
            }
            if plus_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        // Workspace count badge
        if workspace_count > 0 {
            // On native, shift badge left when "+" button is visible (hover/selected)
            #[cfg(not(target_arch = "wasm32"))]
            let badge_x = if hovered || is_selected {
                right_edge - 24.0
            } else {
                right_edge
            };
            #[cfg(target_arch = "wasm32")]
            let badge_x = right_edge;
            ui.painter().text(
                egui::pos2(badge_x, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                workspace_count.to_string(),
                typography::proportional(9.5),
                text_tertiary.gamma_multiply(0.5),
            );
        }

        // Click on header → toggle collapse
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            return Some(ProjectSidebarResult::ToggleProjectCollapse(
                name.to_string(),
            ));
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    fn render_workspace_row(
        &self,
        ui: &mut egui::Ui,
        item: &SidebarWorkspaceItem,
        is_active: bool,
        is_selected: bool,
        indented: bool,
        accent: Color32,
        text_primary: Color32,
        text_secondary: Color32,
        text_tertiary: Color32,
    ) -> Option<ProjectSidebarResult> {
        let row_height = 36.0;
        let avail_width = ui.available_width();
        let indent = if indented { 12.0 } else { 0.0 };

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(avail_width, row_height), egui::Sense::click());

        // Use rect_contains_pointer so the archive button doesn't cause flicker
        let hovered = ui.rect_contains_pointer(rect);
        let highlighted = is_selected || hovered;

        // ── Background ──
        if is_active {
            let active_bg = accent.gamma_multiply(0.08);
            ui.painter().rect_filled(rect, 0.0, active_bg);
            // Left accent bar
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(2.0, row_height));
            ui.painter().rect_filled(bar, 0.0, accent);
        } else if is_selected {
            // Keyboard selection highlight
            let sel_bg = accent.gamma_multiply(0.06);
            ui.painter().rect_filled(rect, 0.0, sel_bg);
            let bar = egui::Rect::from_min_size(rect.min, Vec2::new(2.0, row_height));
            ui.painter()
                .rect_filled(bar, 0.0, accent.gamma_multiply(0.5));
        } else if hovered {
            let hover_bg = text_primary.gamma_multiply(0.03);
            ui.painter().rect_filled(rect, 0.0, hover_bg);
        }

        // ── Icon ──
        let icon_x = rect.min.x + 14.0 + indent;
        let icon_color = if is_active || is_selected {
            accent
        } else if hovered {
            text_secondary
        } else {
            text_tertiary.gamma_multiply(0.7)
        };
        ui.painter().text(
            egui::pos2(icon_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            egui_nerdfonts::regular::CIRCLE_SMALL,
            egui::FontId::proportional(semantic_icons::SIZE_ITEM),
            icon_color,
        );

        // ── Name ──
        let name_x = icon_x + 22.0;
        let name_color = if is_active {
            text_primary
        } else if highlighted {
            text_primary.gamma_multiply(0.9)
        } else {
            text_secondary
        };

        // ── Archive button (on hover/selected, native only — hidden on WASM demo) ──
        #[cfg(not(target_arch = "wasm32"))]
        let right_pad = if hovered || is_selected { 24.0 } else { 10.0 };
        #[cfg(target_arch = "wasm32")]
        let right_pad = 10.0;

        #[cfg(not(target_arch = "wasm32"))]
        if hovered || is_selected {
            let archive_x = rect.max.x - 16.0;
            let archive_rect = egui::Rect::from_center_size(
                egui::pos2(archive_x, rect.center().y),
                Vec2::new(18.0, 18.0),
            );
            let archive_btn = ui.interact(
                archive_rect,
                ui.id().with(("ws_archive", &item.name)),
                egui::Sense::click(),
            );
            ui.painter().text(
                archive_rect.center(),
                egui::Align2::CENTER_CENTER,
                semantic_icons::action::DELETE,
                egui::FontId::proportional(12.0),
                text_tertiary.gamma_multiply(0.5),
            );
            if archive_btn.clicked() {
                return Some(ProjectSidebarResult::ArchiveWorkspace(item.name.clone()));
            }
            if archive_btn.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                archive_btn.on_hover_text("Delete workspace");
            }
        }

        {
            let max_name_w = avail_width - name_x - right_pad;
            let name_text = truncate_text(&item.name, max_name_w, typography::SM * 0.55);
            ui.painter().text(
                egui::pos2(name_x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &name_text,
                typography::proportional(typography::SM),
                name_color,
            );
        }

        // Click handling
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if response.clicked() {
            return Some(ProjectSidebarResult::LoadWorkspace(item.name.clone()));
        }

        None
    }

    fn render_footer(
        &mut self,
        ui: &mut egui::Ui,
        result: &mut ProjectSidebarResult,
        text_tertiary: Color32,
    ) {
        // Separator
        let sep = self.theme.border_subtle().gamma_multiply(0.4);
        ui.painter().hline(
            ui.available_rect_before_wrap().x_range(),
            ui.cursor().min.y,
            egui::Stroke::new(1.0, sep),
        );
        ui.add_space(4.0);

        // Compact icon bar: [+ project] ... [settings]
        // On WASM demo, hide "Add project" — only show settings
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // New project (native only — hidden on WASM demo)
            #[cfg(not(target_arch = "wasm32"))]
            {
                let icon_color = text_tertiary.gamma_multiply(0.6);
                let proj_icon = ui.add(
                    egui::Button::new(
                        RichText::new(semantic_icons::file::FOLDER_PLUS)
                            .color(icon_color)
                            .font(egui::FontId::proportional(semantic_icons::SIZE_ITEM)),
                    )
                    .frame(false),
                );
                let label = ui.label(
                    RichText::new("Add project")
                        .color(icon_color)
                        .font(typography::proportional(typography::XS)),
                );
                let proj_clicked = proj_icon.clicked() || label.clicked();
                let proj_hovered = proj_icon.hovered() || label.hovered();
                if proj_clicked {
                    *result = ProjectSidebarResult::CreateProject;
                }
                if proj_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }

            // Push settings to the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                let settings_btn = ui.add(
                    egui::Button::new(
                        RichText::new(semantic_icons::action::SETTINGS)
                            .color(text_tertiary)
                            .font(egui::FontId::proportional(semantic_icons::SIZE_ITEM)),
                    )
                    .frame(false),
                );
                if settings_btn.clicked() {
                    *result = ProjectSidebarResult::OpenSettings;
                }
                if settings_btn.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    settings_btn.on_hover_text("Settings");
                }
            });
        });

        ui.add_space(4.0);
    }
}

/// Rough text truncation based on estimated character width.
fn truncate_text(text: &str, max_width: f32, avg_char_width: f32) -> String {
    let max_chars = (max_width / avg_char_width) as usize;
    if text.len() <= max_chars {
        text.to_string()
    } else if max_chars > 3 {
        format!("{}...", &text[..max_chars - 3])
    } else {
        "...".to_string()
    }
}
