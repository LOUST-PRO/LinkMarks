//! Forward-only SQLite migrator for the LinkMarks store.
//!
//! Each migration is a `Migration { version, description, sql }` entry in
//! the `MIGRATIONS` slice. `migrate(conn)` runs every un-applied migration
//! inside a single transaction and stamps `PRAGMA user_version` with the
//! final version. Re-running `migrate` on an up-to-date DB is a no-op.
//!
//! The migrator does **not** support downgrades. Forward-only matches the
//! LinkMarks operational model (snapshot + restart beats destructive
//! rollback). Detecting a newer schema than the highest known migration
//! returns [`CoreError::Storage`] with a `newer schema` prefix so callers
//! can surface a clear upgrade error.

use crate::errors::CoreError;
use rusqlite::Connection;

/// One forward migration. `sql` may contain any number of statements
/// separated by `;`. `version` is monotonically increasing and is
/// stamped into `PRAGMA user_version` after the migration commits.
#[derive(Debug, Clone)]
pub struct Migration {
    /// Monotonic version (1, 2, 3…). Stored in `schema_migrations.version`.
    pub version: i64,
    /// Human-readable description. Stored in `schema_migrations.description`.
    pub description: &'static str,
    /// SQL statements (semicolon-separated) to apply. May include
    /// `CREATE TABLE`, `CREATE INDEX`, etc.
    pub sql: &'static str,
}

/// All known migrations. Append-only.
///
/// Each migration must use `IF NOT EXISTS` / `CREATE OR REPLACE` where
/// re-running on a partially-migrated DB is a possibility (e.g. when a
/// previous run committed the SQL but crashed before stamping
/// `user_version`).
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "initial: bookmarks + tags",
    sql: MIGRATION_001_SQL,
}];

/// SQL body for migration version 1. Stored separately so the const
/// initializers above stay readable.
pub const MIGRATION_001_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL,
    description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS bookmarks (
    id TEXT PRIMARY KEY,
    original_url TEXT NOT NULL,
    canonical_url TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    description TEXT,
    collection TEXT,
    source_kind TEXT NOT NULL,
    source_id TEXT,
    external_id TEXT,
    added_at INTEGER NOT NULL,
    last_seen_at INTEGER NOT NULL,
    raw TEXT,
    archived INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX IF NOT EXISTS bookmarks_canonical_url_uidx
    ON bookmarks (canonical_url) WHERE archived = 0;

CREATE INDEX IF NOT EXISTS bookmarks_last_seen_idx ON bookmarks (last_seen_at DESC);
CREATE INDEX IF NOT EXISTS bookmarks_collection_idx ON bookmarks (collection);

CREATE TABLE IF NOT EXISTS tags (
    bookmark_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (bookmark_id, tag),
    FOREIGN KEY (bookmark_id) REFERENCES bookmarks(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS tags_tag_idx ON tags (tag);
"#;

/// Highest migration version known to this binary. The migrator will
/// reject any DB whose `user_version` exceeds this constant.
pub const MAX_SUPPORTED_VERSION: i64 = 1;

/// Run every un-applied migration in a single transaction and stamp
/// `PRAGMA user_version` with the final version.
///
/// Returns the number of migrations actually applied (0 when the DB is
/// already up-to-date).
pub fn migrate(conn: &Connection) -> Result<usize, CoreError> {
    let current = current_user_version(conn)?;
    if current > MAX_SUPPORTED_VERSION {
        return Err(CoreError::Storage(format!(
            "newer schema detected: db version {current} > supported {MAX_SUPPORTED_VERSION}. \
             upgrade LinkMarks before opening this store"
        )));
    }

    let mut applied = 0usize;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| CoreError::Storage(format!("begin tx: {e}")))?;

    for mig in MIGRATIONS {
        if mig.version <= current {
            continue;
        }

        tx.execute_batch(mig.sql)
            .map_err(|e| CoreError::Storage(format!("migration v{}: {e}", mig.version)))?;

        // Record the application. `INSERT OR IGNORE` keeps us idempotent
        // if a partial previous run already wrote the row but did not
        // bump `user_version`.
        let applied_at = unix_now_secs();
        tx.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, applied_at, description) \
             VALUES (?1, ?2, ?3)",
            rusqlite::params![mig.version, applied_at, mig.description],
        )
        .map_err(|e| CoreError::Storage(format!("record migration v{}: {e}", mig.version)))?;

        applied += 1;
    }

    if applied > 0 {
        // Stamp the final version. `user_version` is a SQLite built-in
        // monotonic counter, suitable as our migration marker.
        let final_version = MIGRATIONS.last().expect("non-empty MIGRATIONS").version;
        tx.pragma_update(None, "user_version", final_version)
            .map_err(|e| CoreError::Storage(format!("set user_version: {e}")))?;
    }

    tx.commit()
        .map_err(|e| CoreError::Storage(format!("commit migrations: {e}")))?;
    Ok(applied)
}

/// Read the current `PRAGMA user_version` from the connection.
///
/// Returns 0 on a fresh DB (before the pragma is set).
pub fn current_user_version(conn: &Connection) -> Result<i64, CoreError> {
    conn.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
        .map_err(|e| CoreError::Storage(format!("read user_version: {e}")))
}

/// Returns the list of versions currently recorded in `schema_migrations`.
///
/// Used by tests; not part of the public migrator contract.
pub fn applied_versions(conn: &Connection) -> Result<Vec<i64>, CoreError> {
    let mut stmt = conn
        .prepare("SELECT version FROM schema_migrations ORDER BY version ASC")
        .map_err(|e| CoreError::Storage(format!("prepare applied_versions: {e}")))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, i64>(0))
        .map_err(|e| CoreError::Storage(format!("query applied_versions: {e}")))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| CoreError::Storage(format!("decode version: {e}")))?);
    }
    Ok(out)
}

#[inline]
fn unix_now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage;

    #[test]
    fn fresh_db_runs_migration_001() {
        let conn = storage::open_in_memory().unwrap();
        assert_eq!(current_user_version(&conn).unwrap(), 0);
        let applied = migrate(&conn).unwrap();
        assert_eq!(applied, 1);
        assert_eq!(current_user_version(&conn).unwrap(), 1);
        assert_eq!(applied_versions(&conn).unwrap(), vec![1]);
    }

    #[test]
    fn migrate_is_idempotent_on_up_to_date_db() {
        let conn = storage::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let applied = migrate(&conn).unwrap();
        assert_eq!(applied, 0, "second run must be a no-op");
        assert_eq!(current_user_version(&conn).unwrap(), 1);
    }

    #[test]
    fn reject_db_newer_than_supported() {
        let conn = storage::open_in_memory().unwrap();
        // Simulate a future migration by directly stamping user_version
        // above MAX_SUPPORTED_VERSION.
        conn.pragma_update(None, "user_version", MAX_SUPPORTED_VERSION + 5)
            .unwrap();
        let err = migrate(&conn).unwrap_err();
        match err {
            CoreError::Storage(msg) => assert!(
                msg.contains("newer schema"),
                "expected 'newer schema' error, got: {msg}"
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
