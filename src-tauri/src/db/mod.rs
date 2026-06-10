//! Database connection management and migrations.
//!
//! A single SQLite connection guarded by a mutex is sufficient for a desktop
//! app of this size; all access goes through `Db::with`.

pub mod repo;

use crate::error::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Ordered list of migrations. Append new migrations at the end; never edit
/// an existing one after release.
const MIGRATIONS: &[(&str, &str)] = &[("001_initial", include_str!("migrations/001_initial.sql"))];

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::from_connection(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Db {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let applied: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (idx, (name, sql)) in MIGRATIONS.iter().enumerate() {
            let version = idx as i64 + 1;
            if version > applied {
                tracing::info!(migration = name, "applying database migration");
                conn.execute_batch(&format!(
                    "BEGIN; {sql}; PRAGMA user_version = {version}; COMMIT;"
                ))?;
            }
        }
        Ok(())
    }

    /// Run `f` with exclusive access to the connection.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn.lock().unwrap();
        f(&conn)
    }

    /// Checkpoint the WAL so the main database file is safe to copy.
    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly() {
        let db = Db::open_in_memory().unwrap();
        let version: i64 = db
            .with(|c| Ok(c.query_row("PRAGMA user_version", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn migrations_are_idempotent() {
        let db = Db::open_in_memory().unwrap();
        // Re-running against an already-migrated database must be a no-op.
        db.migrate().unwrap();
        let tables: i64 = db
            .with(|c| {
                Ok(c.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='games'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(tables, 1);
    }

    #[test]
    fn migrations_persist_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        {
            let db = Db::open(&path).unwrap();
            db.with(|c| {
                c.execute(
                    "INSERT INTO platforms (id, name, short_name) VALUES ('nes', 'NES', 'NES')",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        }
        let db = Db::open(&path).unwrap();
        let count: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM platforms", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(count, 1);
    }
}
