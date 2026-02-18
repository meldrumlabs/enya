//! Connection management types and rendering for the SQL pane.

use egui::{Color32, RichText};
use enya_datafusion::{ConnectionState, FlightClient, QueryEvent, Session, TableInfo};
use rustc_hash::FxHashSet;
use tokio::sync::mpsc;

use crate::components::util::id_generator::next_id_usize;
use crate::ui::semantic_icons::{action, category, file, nav};
use crate::ui::theme::AppTheme;

/// Unique identifier for a saved connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(usize);

impl ConnectionId {
    pub(super) fn new() -> Self {
        Self(next_id_usize())
    }
}

/// A saved database connection configuration.
#[derive(Debug, Clone)]
pub struct SavedConnection {
    /// Unique identifier.
    pub id: ConnectionId,
    /// Display name (e.g., "Production", "Staging", "Local").
    pub name: String,
    /// Flight SQL endpoint URL.
    pub endpoint: String,
    /// Connection state.
    pub state: ConnectionState,
    /// Tables discovered from this connection.
    pub tables: Vec<TableInfo>,
    /// Whether this connection is the currently active one.
    pub active: bool,
}

impl SavedConnection {
    pub(super) fn new(name: &str, endpoint: &str) -> Self {
        Self {
            id: ConnectionId::new(),
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            state: ConnectionState::Disconnected,
            tables: Vec::new(),
            active: false,
        }
    }
}

/// State for the connection tree sidebar.
#[derive(Debug, Clone, Default)]
pub struct ConnectionTreeState {
    /// IDs of expanded connections (showing their tables).
    pub expanded: FxHashSet<ConnectionId>,
    /// Currently selected item in the tree.
    pub selected: Option<TreeSelection>,
}

/// What is selected in the connection tree.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Variants for future sidebar tree interaction
pub enum TreeSelection {
    /// A connection is selected.
    Connection(ConnectionId),
    /// A table within a connection is selected.
    Table {
        connection: ConnectionId,
        table: String,
    },
}

/// Backend for SQL execution - either local DataFusion or remote Flight.
#[allow(dead_code)] // Local variant will be used for file-based queries
pub(super) enum SqlBackend {
    /// Local DataFusion session (for file queries).
    Local {
        session: Session,
        event_rx: mpsc::Receiver<QueryEvent>,
    },
    /// Remote Flight SQL connection.
    Flight {
        #[allow(dead_code)] // Client stored for reconnection; queries use endpoint
        client: Box<FlightClient>, // Boxed to avoid large enum variant warning
        tables: Vec<TableInfo>,
    },
}

// =============================================================================
// Connection UI Actions
// =============================================================================

/// Actions that can be triggered by connection UI interactions.
#[derive(Debug, Clone)]
pub enum ConnectionAction {
    /// Connect to a saved connection.
    Connect(ConnectionId),
    /// Disconnect a connection.
    Disconnect(ConnectionId),
    /// Set a connection as active.
    SetActive(ConnectionId),
    /// Remove a connection.
    Remove(ConnectionId),
    /// Toggle expanded state of a connection.
    ToggleExpanded(ConnectionId),
    /// Open the full settings page to manage connections.
    OpenSettings,
    /// Close the connection popup.
    ClosePopup,
    /// Toggle plan viewer visibility.
    TogglePlanViewer,
    /// Insert table name into input.
    InsertTableName(String),
}

/// Snapshot of connection data for rendering (avoids borrow issues).
#[derive(Clone)]
pub struct ConnectionSnapshot {
    pub id: ConnectionId,
    pub name: String,
    pub state: ConnectionState,
    pub active: bool,
    pub tables: Vec<TableInfo>,
}

impl From<&SavedConnection> for ConnectionSnapshot {
    fn from(conn: &SavedConnection) -> Self {
        Self {
            id: conn.id,
            name: conn.name.clone(),
            state: conn.state.clone(),
            active: conn.active,
            tables: conn.tables.clone(),
        }
    }
}

// =============================================================================
// Connection Popup Rendering
// =============================================================================

/// Render the connection dropdown popup.
/// Returns actions to be handled by the caller.
pub fn render_connection_popup(
    ui: &mut egui::Ui,
    theme: AppTheme,
    connections: &[ConnectionSnapshot],
) -> Vec<ConnectionAction> {
    let mut actions = Vec::new();

    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    egui::Area::new(egui::Id::new("connection_popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(ui.available_width() - 250.0, 60.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(theme.bg_elevated())
                .stroke(egui::Stroke::new(1.0, theme.border_default()))
                .corner_radius(8.0)
                .shadow(egui::epaint::Shadow {
                    spread: 0,
                    blur: 16,
                    color: Color32::from_black_alpha(40),
                    offset: [0, 4],
                })
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_min_width(220.0);

                    // Header
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Connections")
                                .color(text_secondary)
                                .size(11.0)
                                .strong(),
                        );
                    });

                    ui.add_space(8.0);

                    // Connection list
                    if connections.is_empty() {
                        ui.label(
                            RichText::new("No connections yet")
                                .color(text_secondary.gamma_multiply(0.6))
                                .size(11.0),
                        );
                    } else {
                        for conn in connections {
                            let is_connected = matches!(conn.state, ConnectionState::Connected);
                            let is_connecting = matches!(conn.state, ConnectionState::Connecting);

                            let status_color = if is_connected {
                                theme.semantic_success()
                            } else if is_connecting {
                                accent
                            } else {
                                text_secondary.gamma_multiply(0.4)
                            };

                            let row_bg = if conn.active {
                                accent.gamma_multiply(0.1)
                            } else {
                                Color32::TRANSPARENT
                            };

                            let row = egui::Frame::new()
                                .fill(row_bg)
                                .corner_radius(4.0)
                                .inner_margin(egui::Margin::symmetric(8, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if is_connecting {
                                            ui.spinner();
                                        } else {
                                            ui.label(
                                                RichText::new("●").color(status_color).size(8.0),
                                            );
                                        }
                                        ui.add_space(8.0);

                                        let name_color =
                                            if conn.active { accent } else { text_primary };
                                        ui.label(
                                            RichText::new(&conn.name).color(name_color).size(12.0),
                                        );
                                    });
                                });

                            if row.response.clicked() {
                                if is_connected {
                                    actions.push(ConnectionAction::SetActive(conn.id));
                                } else if !is_connecting {
                                    actions.push(ConnectionAction::Connect(conn.id));
                                }
                                actions.push(ConnectionAction::ClosePopup);
                            }

                            row.response.context_menu(|ui| {
                                if is_connected && ui.button("Disconnect").clicked() {
                                    actions.push(ConnectionAction::Disconnect(conn.id));
                                    ui.close();
                                }
                                if ui.button("Remove").clicked() {
                                    actions.push(ConnectionAction::Remove(conn.id));
                                    ui.close();
                                }
                            });
                        }
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Manage in Settings link
                    let settings_btn = ui.add(
                        egui::Button::new(
                            RichText::new(format!("{} Manage in Settings", nav::SETTINGS))
                                .color(accent)
                                .size(11.0),
                        )
                        .fill(Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .min_size(egui::vec2(200.0, 24.0)),
                    );
                    if settings_btn.clicked() {
                        actions.push(ConnectionAction::OpenSettings);
                        actions.push(ConnectionAction::ClosePopup);
                    }
                });

            // Close popup when clicking outside
            if ui.input(|i| i.pointer.any_click()) {
                let popup_rect = ui.min_rect();
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if !popup_rect.contains(pos) {
                        actions.push(ConnectionAction::ClosePopup);
                    }
                }
            }
        });

    actions
}

// =============================================================================
// Connection Tree Rendering
// =============================================================================

/// Render the connection tree sidebar.
/// Returns actions to be handled by the caller.
pub fn render_connection_tree(
    ui: &mut egui::Ui,
    theme: AppTheme,
    connections: &[ConnectionSnapshot],
    expanded: &FxHashSet<ConnectionId>,
    sidebar_width: f32,
    show_plan_viewer: bool,
) -> Vec<ConnectionAction> {
    let mut actions = Vec::new();

    let text_primary = theme.text_primary();
    let text_secondary = theme.text_secondary();
    let accent = theme.accent_primary();

    let available = ui.available_size();

    egui::Frame::new()
        .fill(theme.bg_elevated())
        .inner_margin(0.0)
        .show(ui, |ui| {
            ui.set_min_size(available);

            ui.vertical(|ui| {
                // Header
                egui::Frame::new()
                    .fill(theme.bg_surface())
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("CONNECTIONS")
                                    .color(text_secondary)
                                    .size(10.0)
                                    .strong(),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Plan viewer toggle
                                    let plan_color = if show_plan_viewer {
                                        accent
                                    } else {
                                        text_secondary
                                    };
                                    let plan_btn = ui.add(
                                        egui::Button::new(
                                            RichText::new(nav::TREE).color(plan_color).size(12.0),
                                        )
                                        .fill(if show_plan_viewer {
                                            accent.gamma_multiply(0.15)
                                        } else {
                                            Color32::TRANSPARENT
                                        })
                                        .stroke(egui::Stroke::NONE)
                                        .corner_radius(4.0)
                                        .min_size(egui::vec2(24.0, 20.0)),
                                    );
                                    if plan_btn.clicked() {
                                        actions.push(ConnectionAction::TogglePlanViewer);
                                    }
                                    plan_btn.on_hover_text("Toggle plan viewer");
                                },
                            );
                        });
                    });

                // Connection list
                egui::ScrollArea::vertical()
                    .id_salt("connection_tree")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_width(sidebar_width - 16.0);
                        ui.add_space(8.0);

                        if connections.is_empty() {
                            // Empty state
                            ui.vertical_centered(|ui| {
                                ui.add_space(40.0);
                                ui.label(
                                    RichText::new(category::DATAFUSION)
                                        .color(text_secondary.gamma_multiply(0.5))
                                        .size(32.0),
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("No connections")
                                        .color(text_secondary)
                                        .size(12.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Add a connection to get started")
                                        .color(text_secondary.gamma_multiply(0.7))
                                        .size(10.0),
                                );
                            });
                        } else {
                            // Render each connection
                            for conn in connections {
                                let item_actions = render_connection_item(
                                    ui,
                                    theme,
                                    conn,
                                    expanded.contains(&conn.id),
                                    text_primary,
                                    text_secondary,
                                    accent,
                                );
                                actions.extend(item_actions);
                            }
                        }

                        ui.add_space(16.0);
                    });

                // Add connection button at bottom
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    egui::Frame::new()
                        .fill(theme.bg_surface())
                        .inner_margin(egui::Margin::symmetric(8, 8))
                        .show(ui, |ui| {
                            let settings_btn = ui.add(
                                egui::Button::new(
                                    RichText::new(format!("{} Manage in Settings", nav::SETTINGS))
                                        .color(accent)
                                        .size(11.0),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(egui::Stroke::new(1.0, accent.gamma_multiply(0.3)))
                                .corner_radius(4.0)
                                .min_size(egui::vec2(sidebar_width - 24.0, 28.0)),
                            );
                            if settings_btn.clicked() {
                                actions.push(ConnectionAction::OpenSettings);
                            }
                        });
                });
            });
        });

    actions
}

/// Render a single connection item in the tree.
fn render_connection_item(
    ui: &mut egui::Ui,
    theme: AppTheme,
    conn: &ConnectionSnapshot,
    is_expanded: bool,
    text_primary: Color32,
    text_secondary: Color32,
    accent: Color32,
) -> Vec<ConnectionAction> {
    let mut actions = Vec::new();

    let is_connected = matches!(conn.state, ConnectionState::Connected);
    let is_connecting = matches!(conn.state, ConnectionState::Connecting);

    // Connection row
    let row_bg = if conn.active {
        accent.gamma_multiply(0.1)
    } else {
        Color32::TRANSPARENT
    };

    egui::Frame::new()
        .fill(row_bg)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Expand/collapse arrow
                let arrow = if is_expanded {
                    nav::EXPAND
                } else {
                    nav::COLLAPSE
                };
                let arrow_btn = ui.add(
                    egui::Button::new(RichText::new(arrow).color(text_secondary).size(10.0))
                        .fill(Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .min_size(egui::vec2(16.0, 16.0)),
                );
                if arrow_btn.clicked() {
                    actions.push(ConnectionAction::ToggleExpanded(conn.id));
                }

                // Connection status indicator
                let status_color = if is_connected {
                    theme.semantic_success()
                } else if is_connecting {
                    accent
                } else {
                    text_secondary.gamma_multiply(0.5)
                };

                if is_connecting {
                    ui.spinner();
                } else {
                    ui.label(RichText::new("●").color(status_color).size(8.0));
                }

                ui.add_space(4.0);

                // Connection name (clickable to select/activate)
                let name_color = if conn.active { accent } else { text_primary };
                let name_response = ui.add(
                    egui::Label::new(RichText::new(&conn.name).color(name_color).size(12.0))
                        .selectable(false)
                        .sense(egui::Sense::click()),
                );

                if name_response.clicked() && is_connected {
                    actions.push(ConnectionAction::SetActive(conn.id));
                }

                if name_response.double_clicked() && !is_connected && !is_connecting {
                    actions.push(ConnectionAction::Connect(conn.id));
                }

                // Context menu
                name_response.context_menu(|ui| {
                    if is_connected {
                        if ui.button("Disconnect").clicked() {
                            actions.push(ConnectionAction::Disconnect(conn.id));
                            ui.close();
                        }
                        if !conn.active && ui.button("Set as Active").clicked() {
                            actions.push(ConnectionAction::SetActive(conn.id));
                            ui.close();
                        }
                    } else if !is_connecting && ui.button("Connect").clicked() {
                        actions.push(ConnectionAction::Connect(conn.id));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Remove").clicked() {
                        actions.push(ConnectionAction::Remove(conn.id));
                        ui.close();
                    }
                });
            });
        });

    // Expanded tables
    if is_expanded && is_connected {
        ui.indent(format!("tables_{:?}", conn.id), |ui| {
            if conn.tables.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new("No tables")
                            .color(text_secondary.gamma_multiply(0.7))
                            .size(10.0)
                            .italics(),
                    );
                });
            } else {
                for table in &conn.tables {
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.label(RichText::new(file::DATA).color(text_secondary).size(10.0));
                        ui.add_space(4.0);

                        let table_response = ui.add(
                            egui::Label::new(
                                RichText::new(&table.name).color(text_secondary).size(11.0),
                            )
                            .selectable(false)
                            .sense(egui::Sense::click()),
                        );

                        // Double-click to insert table name into query
                        if table_response.double_clicked() {
                            actions.push(ConnectionAction::InsertTableName(table.name.clone()));
                        }
                        table_response.on_hover_text("Double-click to insert into query");
                    });
                }
            }
        });
    }

    actions
}
