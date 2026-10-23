use crate::{
    error::{ApiError, Result},
    model::Snapshot,
};
use rusqlite::{Connection, params};
use std::{
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

#[derive(Clone)]
pub struct Database(pub Arc<Mutex<Connection>>);

#[derive(Clone, Copy)]
pub enum JournalMode {
    Wal,
    Delete,
}

impl JournalMode {
    pub fn parse(value: &str) -> std::result::Result<Self, &'static str> {
        match value {
            "WAL" | "wal" => Ok(Self::Wal),
            "DELETE" | "delete" => Ok(Self::Delete),
            _ => Err("LITTLEWARDROBE_SQLITE_JOURNAL_MODE must be WAL or DELETE"),
        }
    }
}
impl Database {
    #[cfg(test)]
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with_journal(path, JournalMode::Wal)
    }
    pub fn open_with_journal(path: &Path, journal: JournalMode) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let (mode, synchronous) = match journal {
            JournalMode::Wal => ("WAL", "FULL"),
            // NFS cannot share a WAL index. EXTRA also syncs the directory after
            // deleting a rollback journal, making a completed commit durable.
            JournalMode::Delete => ("DELETE", "EXTRA"),
        };
        let actual: String =
            conn.query_row(&format!("PRAGMA journal_mode={mode}"), [], |row| row.get(0))?;
        if !actual.eq_ignore_ascii_case(mode) {
            return Err(ApiError::internal(
                "SQLite could not activate the configured journal mode",
            ));
        }
        conn.execute_batch(&format!(
            "PRAGMA foreign_keys=ON; PRAGMA synchronous={synchronous};"
        ))?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL)")?;
        let version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version),0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        if version > 9 {
            return Err(ApiError::internal(
                "This database was created by a newer Little Wardrobe version. Restore a compatible backup before downgrading.",
            ));
        }
        if version == 0 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/001_initial.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 2 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/002_companion.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 3 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/003_media_settings.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 4 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/004_catalog_settings.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 5 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/005_item_icons.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 6 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/006_clothing_photos.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 7 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/007_photo_intake.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 8 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/008_quick_add.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        if version < 9 {
            conn.execute_batch("BEGIN IMMEDIATE")?;
            conn.execute_batch(include_str!("../migrations/009_oidc.sql"))?;
            conn.execute_batch("COMMIT")?;
        }
        conn.execute(
            "DELETE FROM companion_jobs WHERE deadline < unixepoch()",
            [],
        )?;
        conn.execute("DELETE FROM sessions WHERE expires_at < unixepoch()", [])?;
        conn.execute("DELETE FROM invitations WHERE expires_at < unixepoch()", [])?;
        conn.execute("DELETE FROM oidc_logins WHERE expires_at < unixepoch()", [])?;
        conn.execute(
            "DELETE FROM ai_proposals WHERE created_at < unixepoch()-86400",
            [],
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self(Arc::new(Mutex::new(conn))))
    }
    pub fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        match self.0.lock() {
            Ok(guard) => Ok(guard),
            Err(poisoned) => {
                tracing::error!("Recovering the database connection after a panic");
                let guard = poisoned.into_inner();
                if !guard.is_autocommit() {
                    guard.execute_batch("ROLLBACK")?;
                }
                self.0.clear_poison();
                Ok(guard)
            }
        }
    }
    pub fn snapshot(conn: &Connection, household: &str) -> Result<Snapshot> {
        let (revision, data): (u32, String) = conn.query_row(
            "SELECT revision, state FROM households WHERE id=?1",
            [household],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(Snapshot {
            revision,
            state: serde_json::from_str(&data)?,
        })
    }
    pub fn save(conn: &Connection, household: &str, snapshot: &Snapshot) -> Result<()> {
        conn.execute(
            "UPDATE households SET revision=?1, state=?2 WHERE id=?3",
            params![
                snapshot.revision,
                serde_json::to_string(&snapshot.state)?,
                household
            ],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_journal_preserves_wal_data_transactions_backups_and_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("database.sqlite3");
        let database = Database::open(&path).unwrap();
        database.lock().unwrap().execute_batch("CREATE TABLE storage_probe(value BLOB); INSERT INTO storage_probe VALUES (X'001122FF');").unwrap();
        drop(database);

        let database = Database::open_with_journal(&path, JournalMode::Delete).unwrap();
        {
            let mut conn = database.lock().unwrap();
            let mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap();
            assert_eq!(mode, "delete");
            let sync: u32 = conn
                .query_row("PRAGMA synchronous", [], |row| row.get(0))
                .unwrap();
            assert_eq!(sync, 3);
            let transaction = conn.transaction().unwrap();
            transaction
                .execute("DELETE FROM storage_probe", [])
                .unwrap();
            drop(transaction);
        }
        // The separate connection used by --backup can read a running DELETE database.
        let backup = directory.path().join("backup.sqlite3");
        Connection::open(&path)
            .unwrap()
            .backup("main", &backup, None)
            .unwrap();
        drop(database);
        for file in [&path, &backup] {
            let reopened = Database::open_with_journal(file, JournalMode::Delete).unwrap();
            let value: Vec<u8> = reopened
                .lock()
                .unwrap()
                .query_row("SELECT value FROM storage_probe", [], |row| row.get(0))
                .unwrap();
            assert_eq!(value, [0, 17, 34, 255]);
            let integrity: String = reopened
                .lock()
                .unwrap()
                .query_row("PRAGMA integrity_check", [], |row| row.get(0))
                .unwrap();
            assert_eq!(integrity, "ok");
        }
        assert!(!path.with_file_name("database.sqlite3-wal").exists());
        assert!(!path.with_file_name("database.sqlite3-shm").exists());
    }
}
