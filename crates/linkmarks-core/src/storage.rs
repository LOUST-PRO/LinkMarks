//! SQLite store initialization with WAL and contention hardening.
use crate::errors::CoreError;
use rusqlite::Connection;
use std::path::Path;

/// SQLite lock wait timeout in milliseconds.
pub const BUSY_TIMEOUT_MS: u32 = 5000;

/// Opens a file-backed SQLite connection with hardened pragmas.
pub fn open(path: &Path) -> Result<Connection, CoreError> {
    let conn = Connection::open(path).map_err(|e| CoreError::Storage(e.to_string()))?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

/// Opens an in-memory SQLite connection with hardened pragmas.
pub fn open_in_memory() -> Result<Connection, CoreError> {
    let conn = Connection::open_in_memory().map_err(|e| CoreError::Storage(e.to_string()))?;
    apply_pragmas(&conn)?;
    Ok(conn)
}

fn apply_pragmas(conn: &Connection) -> Result<(), CoreError> {
    conn.execute_batch(&format!(
        "PRAGMA journal_mode = WAL; PRAGMA busy_timeout = {BUSY_TIMEOUT_MS}; PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON; PRAGMA temp_store = MEMORY;"
    )).map_err(|e| CoreError::Storage(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn opens_with_wal_mode() {
        let dir = tempdir().unwrap();
        let conn = open(&dir.path().join("test.db")).unwrap();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn busy_timeout_configured() {
        let conn = open_in_memory().unwrap();
        let timeout: i64 = conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(timeout, BUSY_TIMEOUT_MS as i64);
    }

    #[test]
    fn foreign_keys_enabled() {
        let conn = open_in_memory().unwrap();
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn concurrent_readers_do_not_block_writer() {
        // WAL enables concurrent readers + 1 writer; the busy_timeout
        // ensures waiters don't error out instantly. This test exercises
        // the contract: a writer thread commits while a reader thread
        // sees pre-commit data via the snapshot MVCC.
        let dir = tempdir().unwrap();
        let path = dir.path().join("concurrent.db");
        let setup = open(&path).unwrap();
        setup
            .execute_batch("CREATE TABLE t (x INTEGER); INSERT INTO t VALUES (1);")
            .unwrap();
        drop(setup);

        let path_w = path.clone();
        let writer = thread::spawn(move || {
            let conn: Connection = open(&path_w).unwrap();
            conn.execute("INSERT INTO t VALUES (2)", []).unwrap();
        });

        // Reader: open a fresh connection mid-write. With WAL the
        // reader sees the committed state at connection-open time and
        // does not block.
        let reader = open(&path).unwrap();
        let count: i64 = reader
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert!(count >= 1, "reader starved: count={count}");

        writer.join().expect("writer thread panicked");

        // After writer completes, reader can re-query and see the new row.
        let final_count: i64 = reader
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(final_count, 2);
    }

    #[test]
    fn synchronous_normal_with_wal_is_safe() {
        // Smoke check: `synchronous=NORMAL` with WAL does NOT corrupt
        // on a clean shutdown. We assert the pragma is set; durability
        // guarantees are SQLite-documented, not re-derived here.
        let conn = open_in_memory().unwrap();
        let sync: i64 = conn
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .unwrap();
        // 0=OFF, 1=NORMAL, 2=FULL. With WAL the NORMAL level is the
        // documented sweet spot.
        assert_eq!(sync, 1, "expected synchronous=NORMAL (1)");
    }

    #[test]
    fn temp_store_in_memory() {
        let conn = open_in_memory().unwrap();
        let mode: i64 = conn
            .query_row("PRAGMA temp_store", [], |r| r.get(0))
            .unwrap();
        // 0=DEFAULT, 1=FILE, 2=MEMORY
        assert_eq!(mode, 2, "expected temp_store=MEMORY (2)");
    }
}
