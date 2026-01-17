//! Connection management types for the SQL pane.

use enya_datafusion::{ConnectionState, FlightClient, QueryEvent, Session, TableInfo};
use rustc_hash::FxHashSet;
use tokio::sync::mpsc;

use crate::components::util::id_generator::next_id_usize;

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
    /// Whether the "Add Connection" dialog is open.
    pub show_add_dialog: bool,
    /// Name input for new connection dialog.
    pub new_conn_name: String,
    /// Endpoint input for new connection dialog.
    pub new_conn_endpoint: String,
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
