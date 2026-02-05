//! SQLite state storage for the Enya agent.
//!
//! Stores persistent state that survives agent restarts: watch configurations,
//! events (alerts, resolutions), and eventually investigations and notifications.

use std::path::Path;

use parking_lot::Mutex;

use rusqlite::{Connection, params};
use serde::Serialize;

/// SQLite database handle for agent state.
///
/// Uses a `Mutex<Connection>` so it can be shared across async tasks via `Arc<Db>`.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    /// Open (or create) the database at the given path and run migrations.
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;

        // WAL mode for concurrent reads while the agent writes.
        conn.pragma_update(None, "journal_mode", "wal")?;

        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Open an in-memory database (for tests).
    #[cfg(test)]
    pub fn open_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Run schema migrations.
    fn migrate(&self) -> rusqlite::Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER NOT NULL
            )",
        )?;

        let current: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if current < 1 {
            Self::migrate_v1(&conn)?;
        }

        Ok(())
    }

    /// v1: watches and events tables.
    fn migrate_v1(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE watches (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL,
                expression  TEXT NOT NULL,
                threshold_op    TEXT NOT NULL CHECK (threshold_op IN ('above', 'below')),
                threshold_value REAL NOT NULL,
                interval_secs   INTEGER NOT NULL DEFAULT 30,
                sustain_secs    INTEGER,
                endpoint    TEXT NOT NULL,
                enabled     INTEGER NOT NULL DEFAULT 1,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                watch_id    INTEGER NOT NULL REFERENCES watches(id),
                event_type  TEXT NOT NULL CHECK (event_type IN ('alert', 'resolve', 'error')),
                value       REAL,
                message     TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            INSERT INTO schema_version (version) VALUES (1);",
        )?;
        Ok(())
    }

    // -- Watch CRUD --

    /// Insert a new watch, returning its ID.
    pub fn insert_watch(&self, w: &NewWatch) -> rusqlite::Result<i64> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO watches (name, expression, threshold_op, threshold_value,
                                  interval_secs, sustain_secs, endpoint)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                w.name,
                w.expression,
                w.threshold_op,
                w.threshold_value,
                w.interval_secs,
                w.sustain_secs,
                w.endpoint,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// List all enabled watches.
    pub fn list_watches(&self) -> rusqlite::Result<Vec<Watch>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, expression, threshold_op, threshold_value,
                    interval_secs, sustain_secs, endpoint, enabled
             FROM watches WHERE enabled = 1
             ORDER BY id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Watch {
                id: row.get(0)?,
                name: row.get(1)?,
                expression: row.get(2)?,
                threshold_op: row.get(3)?,
                threshold_value: row.get(4)?,
                interval_secs: row.get(5)?,
                sustain_secs: row.get(6)?,
                endpoint: row.get(7)?,
                enabled: row.get(8)?,
            })
        })?;
        rows.collect()
    }

    /// Get a single watch by ID (returns None if not found or disabled).
    pub fn get_watch(&self, id: i64) -> rusqlite::Result<Option<Watch>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, expression, threshold_op, threshold_value,
                    interval_secs, sustain_secs, endpoint, enabled
             FROM watches WHERE id = ?1 AND enabled = 1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Watch {
                id: row.get(0)?,
                name: row.get(1)?,
                expression: row.get(2)?,
                threshold_op: row.get(3)?,
                threshold_value: row.get(4)?,
                interval_secs: row.get(5)?,
                sustain_secs: row.get(6)?,
                endpoint: row.get(7)?,
                enabled: row.get(8)?,
            })
        })?;
        match rows.next() {
            Some(Ok(watch)) => Ok(Some(watch)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }

    /// Disable a watch by ID (soft delete).
    pub fn disable_watch(&self, id: i64) -> rusqlite::Result<bool> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE watches SET enabled = 0, updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(changed > 0)
    }

    // -- Events --

    /// Record a watch event.
    pub fn insert_event(
        &self,
        watch_id: i64,
        event_type: &str,
        value: Option<f64>,
        message: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO events (watch_id, event_type, value, message)
             VALUES (?1, ?2, ?3, ?4)",
            params![watch_id, event_type, value, message],
        )?;
        Ok(())
    }

    /// Get recent events for a watch.
    pub fn recent_events(&self, watch_id: i64, limit: u32) -> rusqlite::Result<Vec<Event>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, watch_id, event_type, value, message, created_at
             FROM events WHERE watch_id = ?1
             ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![watch_id, limit], |row| {
            Ok(Event {
                id: row.get(0)?,
                watch_id: row.get(1)?,
                event_type: row.get(2)?,
                value: row.get(3)?,
                message: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect()
    }
}

/// Parameters for creating a new watch.
pub struct NewWatch<'a> {
    pub name: &'a str,
    pub expression: &'a str,
    pub threshold_op: &'a str,
    pub threshold_value: f64,
    pub interval_secs: u32,
    pub sustain_secs: Option<u32>,
    pub endpoint: &'a str,
}

/// A persisted watch configuration.
#[derive(Debug, Serialize)]
pub struct Watch {
    pub id: i64,
    pub name: String,
    pub expression: String,
    pub threshold_op: String,
    pub threshold_value: f64,
    pub interval_secs: u32,
    pub sustain_secs: Option<u32>,
    pub endpoint: String,
    pub enabled: bool,
}

/// A recorded watch event.
#[derive(Debug, Serialize)]
pub struct Event {
    pub id: i64,
    pub watch_id: i64,
    pub event_type: String,
    pub value: Option<f64>,
    pub message: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_list_watches() {
        let db = Db::open_memory().unwrap();

        let id = db
            .insert_watch(&NewWatch {
                name: "high-cpu",
                expression: "avg(rate(cpu_usage[5m]))",
                threshold_op: "above",
                threshold_value: 0.9,
                interval_secs: 30,
                sustain_secs: Some(300),
                endpoint: "http://prometheus:9090",
            })
            .unwrap();

        assert_eq!(id, 1);

        let watches = db.list_watches().unwrap();
        assert_eq!(watches.len(), 1);
        assert_eq!(watches[0].name, "high-cpu");
        assert_eq!(watches[0].threshold_op, "above");
    }

    #[test]
    fn test_disable_watch() {
        let db = Db::open_memory().unwrap();

        let id = db
            .insert_watch(&NewWatch {
                name: "test",
                expression: "up",
                threshold_op: "below",
                threshold_value: 1.0,
                interval_secs: 60,
                sustain_secs: None,
                endpoint: "http://localhost:9090",
            })
            .unwrap();

        assert_eq!(db.list_watches().unwrap().len(), 1);
        db.disable_watch(id).unwrap();
        assert_eq!(db.list_watches().unwrap().len(), 0);
    }

    #[test]
    fn test_events() {
        let db = Db::open_memory().unwrap();

        let id = db
            .insert_watch(&NewWatch {
                name: "test",
                expression: "up",
                threshold_op: "below",
                threshold_value: 1.0,
                interval_secs: 60,
                sustain_secs: None,
                endpoint: "http://localhost:9090",
            })
            .unwrap();

        db.insert_event(
            id,
            "alert",
            Some(0.5),
            Some("value dropped below threshold"),
        )
        .unwrap();
        db.insert_event(id, "resolve", Some(1.0), None).unwrap();

        let events = db.recent_events(id, 10).unwrap();
        assert_eq!(events.len(), 2);
        // Most recent first
        assert_eq!(events[0].event_type, "resolve");
        assert_eq!(events[1].event_type, "alert");
    }
}
