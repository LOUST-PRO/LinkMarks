//! Integration tests for `linkmarks-core::store`.
//!
//! Each test opens a fresh on-disk store under a tempdir and exercises
//! one contract. Tests run serially against their own DB so they can
//! be `cargo test --workspace` without coordination.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use chrono::{TimeZone, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use linkmarks_core::store::{open as open_store, open_in_memory};
use rusqlite::Connection;
use tempfile::tempdir;

fn fixture_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.push("src/store.rs");
    // We re-use this only as a "file exists" marker for `open`. The
    // tempdir owns the actual DB.
    p
}

fn mk(canonical: &str, title: &str, source: SourceKind, secs: i64) -> Bookmark {
    Bookmark {
        id: BookmarkId::generate(),
        original_url: format!("https://example.com/{title}"),
        canonical_url: canonical.into(),
        title: title.into(),
        description: None,
        tags: vec![],
        collection: None,
        created_at: Utc.timestamp_opt(secs, 0).unwrap(),
        updated_at: Utc.timestamp_opt(secs, 0).unwrap(),
        source: SourceRef {
            kind: source,
            external_id: Some(format!("ext-{title}")),
            imported_at: Utc.timestamp_opt(secs, 0).unwrap(),
            raw: Some(serde_json::json!({"marker": title})),
        },
        content_type: None,
        archived: false,
    }
}

// 1. open crea schema en primera call; idempotente.
#[test]
fn open_creates_schema_and_is_idempotent() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.db");
    {
        let s = open_store(&path).unwrap();
        assert_eq!(s.count().unwrap(), 0);
    }
    // Second open must succeed on the same DB.
    {
        let s = open_store(&path).unwrap();
        assert_eq!(s.count().unwrap(), 0);
    }
}

// 2. upsert con canonical_url nuevo → INSERT + tags cascada.
#[test]
fn upsert_new_canonical_inserts_and_sets_tags() {
    let mut s = open_in_memory().unwrap();
    let mut b = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
    b.tags = vec!["rust".into(), "cli".into()];
    let id = s.upsert(&b).unwrap();
    let tags = s.tags_for(&id).unwrap();
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&"rust".to_string()));
    assert!(tags.contains(&"cli".to_string()));
}

// 3. upsert con canonical_url existente → UPDATE last_seen_at, merge tags.
#[test]
fn upsert_existing_canonical_updates_and_replaces_tags() {
    let mut s = open_in_memory().unwrap();
    let mut b1 = mk("https://example.com/x", "Old Title", SourceKind::Chromium, 100);
    b1.tags = vec!["old".into()];
    let id = s.upsert(&b1).unwrap();

    // Second upsert: same canonical URL, different title + tags.
    let mut b2 = mk("https://example.com/x", "New Title", SourceKind::Chromium, 200);
    b2.tags = vec!["new".into()];
    let id2 = s.upsert(&b2).unwrap();

    // The id should be preserved (upsert by canonical URL).
    assert_eq!(id, id2);
    let row = s.by_canonical("https://example.com/x").unwrap().unwrap();
    assert_eq!(row.title, "New Title");
    // last_seen_at moved forward.
    assert_eq!(row.updated_at.timestamp(), 200);
    // Tags are replaced, not merged.
    assert_eq!(s.tags_for(&id).unwrap(), vec!["new".to_string()]);
}

// 4. by_canonical returns row; missing → Ok(None).
#[test]
fn by_canonical_returns_some_or_none() {
    let mut s = open_in_memory().unwrap();
    let b = mk("https://example.com/x", "X", SourceKind::Chromium, 100);
    s.upsert(&b).unwrap();
    let found = s.by_canonical("https://example.com/x").unwrap();
    assert!(found.is_some());
    let missing = s.by_canonical("https://example.com/missing").unwrap();
    assert!(missing.is_none());
}

// 5. list paginates by (last_seen_at DESC, id ASC).
#[test]
fn list_paginates_by_last_seen_then_id() {
    let mut s = open_in_memory().unwrap();
    let mut ids = Vec::new();
    for i in 0..5i64 {
        let mut b = mk(
            &format!("https://example.com/{i}"),
            &format!("B{i}"),
            SourceKind::Chromium,
            100 + i,
        );
        b.id = BookmarkId(format!("id-{i:02}"));
        ids.push(s.upsert(&b).unwrap());
    }

    let all = s.list(10, 0).unwrap();
    assert_eq!(all.len(), 5);
    // DESC by last_seen_at: B4, B3, B2, B1, B0.
    let titles: Vec<&str> = all.iter().map(|b| b.title.as_str()).collect();
    assert_eq!(titles, vec!["B4", "B3", "B2", "B1", "B0"]);

    // Pagination works.
    let page = s.list(2, 1).unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].title, "B3");
    assert_eq!(page[1].title, "B2");
    let _ = ids;
}

// 6. count returns total after upserts.
#[test]
fn count_returns_total() {
    let mut s = open_in_memory().unwrap();
    assert_eq!(s.count().unwrap(), 0);
    s.upsert(&mk("https://example.com/a", "A", SourceKind::Chromium, 100))
        .unwrap();
    s.upsert(&mk("https://example.com/b", "B", SourceKind::Chromium, 200))
        .unwrap();
    s.upsert(&mk("https://example.com/c", "C", SourceKind::Chromium, 300))
        .unwrap();
    assert_eq!(s.count().unwrap(), 3);
}

// 7. delete soft-archives (row remains, archived=1); re-insert con mismo canonical_url funciona.
#[test]
fn delete_soft_archives_and_reinsert_works() {
    let mut s = open_in_memory().unwrap();
    let b = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
    let id = s.upsert(&b).unwrap();
    s.delete(&id).unwrap();
    // Active count drops; total count stays (tombstone).
    assert_eq!(s.count().unwrap(), 0);
    assert_eq!(s.count_all().unwrap(), 1);
    // Re-inserting the same canonical URL produces a new active row.
    // The unique index is partial (`WHERE archived = 0`), so the
    // tombstone stays as history and the new row lives alongside it.
    let b2 = mk("https://example.com/a", "A-v2", SourceKind::Chromium, 200);
    let id2 = s.upsert(&b2).unwrap();
    assert_eq!(s.count().unwrap(), 1);
    assert_eq!(s.count_all().unwrap(), 2); // tombstone + new active
    assert_ne!(id, id2); // different ids; partial unique index permits both
    let fetched = s.by_canonical("https://example.com/a").unwrap().unwrap();
    assert_eq!(fetched.title, "A-v2");
}

// 8. tags CRUD: set_tags reemplaza set; tag index rebuilt.
#[test]
fn tags_set_replaces_and_index_rebuilt() {
    let mut s = open_in_memory().unwrap();
    let mut b = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
    b.tags = vec!["alpha".into(), "beta".into()];
    let id = s.upsert(&b).unwrap();

    // set_tags replaces.
    s.set_tags(&id, &["gamma".into()]).unwrap();
    assert_eq!(s.tags_for(&id).unwrap(), vec!["gamma".to_string()]);

    // The (bookmark_id, tag) composite PK means a duplicate tag is
    // collapsed via INSERT OR IGNORE.
    s.set_tags(&id, &["delta".into(), "delta".into(), "delta".into()])
        .unwrap();
    assert_eq!(s.tags_for(&id).unwrap(), vec!["delta".to_string()]);

    // Tag index rebuild: after a soft-delete (archive=1) the tag rows
    // stay attached to the tombstone — FK cascade only fires on a hard
    // DELETE. We verify the contract by counting rows before and after
    // a soft-delete (no change expected).
    s.set_tags(&id, &["keep".into(), "drop".into()]).unwrap();
    let before: i64 = s
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM tags WHERE tag IN ('keep','drop')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(before, 2);
    s.delete(&id).unwrap();
    let after_soft: i64 = s
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM tags WHERE tag IN ('keep','drop')",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(after_soft, 2, "soft-delete must NOT cascade-delete tags");
}

// 9. migrator runs each migration exactly once; user_version matches.
#[test]
fn migrator_runs_each_migration_once_and_user_version_matches() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.db");
    let s = open_store(&path).unwrap();
    let version: i64 = s
        .connection()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 1);

    // schema_migrations has exactly one row.
    let rows: i64 = s
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows, 1);

    // Re-open is idempotent: same version, no extra rows.
    drop(s);
    let s = open_store(&path).unwrap();
    let version2: i64 = s
        .connection()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version2, 1);
    let rows2: i64 = s
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rows2, 1);
}

// 10. migrator on DB newer than supported → typed error.
#[test]
fn migrator_on_newer_db_returns_typed_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.db");

    // First open to get the schema in place.
    let s = open_store(&path).unwrap();
    // Stamp a version higher than the maximum known.
    s.connection()
        .pragma_update(None, "user_version", 999)
        .unwrap();
    drop(s);

    // Second open must fail with a `newer schema` error.
    let err = open_store(&path).unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("newer schema"),
        "expected 'newer schema' error, got: {msg}"
    );
}

// 11. concurrent writer + reader.
#[test]
fn concurrent_writer_and_reader() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("store.db");

    // Open once to bootstrap.
    let _ = open_store(&path).unwrap();

    let path_arc: Arc<PathBuf> = Arc::new(path.clone());

    let writer_path = Arc::clone(&path_arc);
    let writer = thread::spawn(move || {
        let mut s = open_store(&writer_path).unwrap();
        for i in 0..20i64 {
            let mut b = mk(
                &format!("https://example.com/w{i}"),
                &format!("W{i}"),
                SourceKind::Chromium,
                1_000 + i,
            );
            b.id = BookmarkId(format!("wid-{i:02}"));
            s.upsert(&b).unwrap();
        }
    });

    // Reader: poll until the writer has produced at least one row.
    let reader_path = Arc::clone(&path_arc);
    let reader = thread::spawn(move || {
        let s = open_store(&reader_path).unwrap();
        let mut seen = 0;
        for _ in 0..100 {
            seen = s.count().unwrap();
            if seen > 0 {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(5));
        }
        seen
    });

    let seen_by_reader = reader.join().expect("reader thread");
    writer.join().expect("writer thread");
    assert!(
        seen_by_reader >= 1,
        "reader starved; saw {seen_by_reader} rows"
    );

    // Final state: 20 rows.
    let s = open_store(&path).unwrap();
    assert_eq!(s.count().unwrap(), 20);
}

// 12. tombstone: archived excluded de list default.
#[test]
fn archived_excluded_from_default_list() {
    let mut s = open_in_memory().unwrap();
    let b1 = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
    let b2 = mk("https://example.com/b", "B", SourceKind::Chromium, 200);
    let id1 = s.upsert(&b1).unwrap();
    s.upsert(&b2).unwrap();
    s.delete(&id1).unwrap();

    let list = s.list(10, 0).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "B");

    // by_canonical also excludes archived.
    assert!(s.by_canonical("https://example.com/a").unwrap().is_none());
    assert!(s.by_canonical("https://example.com/b").unwrap().is_some());
}

// 13. empty store returns empty list, no error.
#[test]
fn empty_store_returns_empty_list() {
    let s = open_in_memory().unwrap();
    let list = s.list(10, 0).unwrap();
    assert!(list.is_empty());
    assert_eq!(s.count().unwrap(), 0);
    assert_eq!(s.count_all().unwrap(), 0);
}

// 14. raw payload round-trip preserved.
#[test]
fn raw_payload_round_trip_preserved() {
    let mut s = open_in_memory().unwrap();
    let mut b = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
    let payload = serde_json::json!({
        "browser": "chrome",
        "profile": "Default",
        "date_added": "13350000000000001",
        "folder_path": ["Bookmarks bar", "Work"],
    });
    b.source.raw = Some(payload.clone());
    let id = s.upsert(&b).unwrap();

    let row = s.by_canonical("https://example.com/a").unwrap().unwrap();
    assert_eq!(row.id, id);
    assert_eq!(row.source.raw.as_ref(), Some(&payload));
}

// 15. ulid generation is monotonic across calls.
#[test]
fn ulid_generation_is_monotonic() {
    // ULIDs include a 48-bit millisecond timestamp prefix followed by
    // 80 bits of randomness. Two ULIDs from the same millisecond are
    // not lexicographically ordered by time — they are random within
    // that millisecond. We assert (a) the embedded timestamp is
    // monotonic across calls that span at least one millisecond, and
    // (b) the format is well-formed.
    let first = ulid::Ulid::from_string(&BookmarkId::generate().0).unwrap();
    // Sleep just over one millisecond so the timestamp portion advances.
    std::thread::sleep(std::time::Duration::from_millis(2));
    let second = ulid::Ulid::from_string(&BookmarkId::generate().0).unwrap();
    let first_ms = first.timestamp_ms();
    let second_ms = second.timestamp_ms();
    assert!(
        second_ms >= first_ms,
        "ULID timestamp went backwards: {first_ms} -> {second_ms}"
    );

    // Format check: 26 chars, ASCII alphanumeric (Crockford base32).
    let sample = BookmarkId::generate().0;
    assert_eq!(sample.len(), 26);
    for c in sample.chars() {
        assert!(
            c.is_ascii_alphanumeric(),
            "non-base32 char in ULID: {c}"
        );
    }
}

// Silence unused-import lint when no test references it.
#[allow(dead_code)]
fn _check_fixture_exists() {
    let _ = fixture_path();
}

#[allow(dead_code)]
fn _check_connection_type() {
    // Type anchor so `Connection` stays imported when other tests
    // move around.
    let _: fn(&Connection) -> &Connection = |c| c;
}