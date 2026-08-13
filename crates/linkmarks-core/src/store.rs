//! SQLite-backed bookmark store.
//!
//! Layered on top of [`crate::storage`] (WAL + contention hardening) and
//! [`crate::migrator`] (forward-only schema versioning). All public
//! methods are blocking; callers wrap them in `tokio::task::spawn_blocking`
//! when they need an async surface.
//!
//! Statements are prepared with `prepare_cached` and re-used by the
//! connection's internal LRU cache. We do **not** hold on to
//! `Statement<'_>` objects (they borrow from the connection); the SQL is
//! the single source of truth.
//!
//! ## Schema ↔ domain mapping
//!
//! `Bookmark.created_at` ↔ `bookmarks.added_at` (epoch seconds).
//! `Bookmark.updated_at` ↔ `bookmarks.last_seen_at` (epoch seconds).
//! `Bookmark.source.kind` ↔ `bookmarks.source_kind` (lowercase string).
//! `Bookmark.source.external_id` ↔ `bookmarks.external_id`.
//! `Bookmark.source.imported_at` is **not** persisted (kept on the source
//! only); the import timestamp on the `SourceRef` is reconstructed as
//! the epoch 0 on read.
//! `Bookmark.source.raw` ↔ `bookmarks.raw` (JSON string).
//! `tags` ↔ `tags` table (one row per `(bookmark_id, tag)`).
//! `content_type` is not persisted in Fase 2; bridges that set it
//! currently do so to `None`.

use crate::errors::CoreError;
use crate::migrator;
use crate::model::{Bookmark, BookmarkId, SourceKind, SourceRef, Tag};
use crate::storage;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

/// Open (and migrate) a file-backed store.
///
/// Creates the parent directory if missing, runs `migrate`, and returns
/// a `Store` ready for CRUD. Idempotent — safe to call repeatedly on
/// the same path.
pub fn open(path: &Path) -> Result<Store, CoreError> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let conn = storage::open(path)?;
    migrator::migrate(&conn)?;
    Ok(Store { conn })
}

/// Open an in-memory store. Used by tests and ephemeral tooling.
pub fn open_in_memory() -> Result<Store, CoreError> {
    let conn = storage::open_in_memory()?;
    migrator::migrate(&conn)?;
    Ok(Store { conn })
}

/// The bookmark store. Wraps a single SQLite connection.
pub struct Store {
    conn: Connection,
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store").finish_non_exhaustive()
    }
}

impl Store {
    /// Borrow the underlying connection. Used by tests that need to
    /// inspect schema state directly.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Insert or update a bookmark. On canonical-URL collision with a
    /// non-archived row, the existing row's `last_seen_at`, title,
    /// description, collection, `external_id`, and `raw` are updated and
    /// tags are replaced by the supplied set.
    ///
    /// Returns the persisted `BookmarkId`. The id of the supplied
    /// bookmark is ignored; the canonical URL drives dedupe.
    pub fn upsert(&mut self, bookmark: &Bookmark) -> Result<BookmarkId, CoreError> {
        let canonical = bookmark.canonical_url.clone();

        let existing_id: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM bookmarks WHERE canonical_url = ?1 AND archived = 0",
                params![canonical],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| CoreError::Storage(format!("lookup canonical: {e}")))?;

        let now = unix_now_secs();
        let persisted_id = match existing_id {
            Some(id) => {
                self.update_existing(&BookmarkId(id), bookmark, now)?;
                BookmarkId(
                    // re-fetch the id we just used
                    self.conn
                        .query_row(
                            "SELECT id FROM bookmarks WHERE canonical_url = ?1 AND archived = 0",
                            params![canonical],
                            |row| row.get::<_, String>(0),
                        )
                        .map_err(|e| CoreError::Storage(format!("refetch id: {e}")))?,
                )
            }
            None => {
                let id = if bookmark.id.0.is_empty() {
                    BookmarkId::generate()
                } else {
                    bookmark.id.clone()
                };
                self.insert_new(&id, bookmark, now)?;
                id
            }
        };

        self.set_tags(&persisted_id, &bookmark.tags)?;
        Ok(persisted_id)
    }

    /// Look up a single bookmark by canonical URL. Excludes archived rows.
    pub fn by_canonical(&self, canonical: &str) -> Result<Option<Bookmark>, CoreError> {
        let row = self
            .conn
            .query_row(
                "\
                 SELECT id, original_url, canonical_url, title, description, collection, \
                  source_kind, source_id, external_id, added_at, last_seen_at, raw, archived \
                 FROM bookmarks WHERE canonical_url = ?1 AND archived = 0",
                params![canonical],
                row_to_bookmark,
            )
            .optional()
            .map_err(|e| CoreError::Storage(format!("by_canonical query: {e}")))?;
        match row {
            Some(mut b) => {
                b.tags = self.tags_for(&b.id)?;
                Ok(Some(b))
            }
            None => Ok(None),
        }
    }

    /// List bookmarks paginated, ordered by `last_seen_at DESC, id ASC`.
    /// Archived rows are excluded.
    pub fn list(&self, limit: usize, offset: usize) -> Result<Vec<Bookmark>, CoreError> {
        let limit = limit.min(i64::MAX as usize) as i64;
        let offset = offset.min(i64::MAX as usize) as i64;
        let mut stmt = self
            .conn
            .prepare_cached(SQL_LIST_ACTIVE)
            .map_err(|e| CoreError::Storage(format!("prepare list: {e}")))?;
        let rows = stmt
            .query_map(params![limit, offset], row_to_bookmark)
            .map_err(|e| CoreError::Storage(format!("list query: {e}")))?;
        let mut out = Vec::new();
        for r in rows {
            let mut b = r.map_err(|e| CoreError::Storage(format!("list row: {e}")))?;
            b.tags = self.tags_for(&b.id)?;
            out.push(b);
        }
        Ok(out)
    }

    /// Total number of **active** bookmarks (excludes archived rows).
    pub fn count(&self) -> Result<i64, CoreError> {
        self.conn
            .query_row(SQL_COUNT_ACTIVE, [], |row| row.get::<_, i64>(0))
            .map_err(|e| CoreError::Storage(format!("count: {e}")))
    }

    /// Total number of bookmarks including archived tombstones.
    pub fn count_all(&self) -> Result<i64, CoreError> {
        self.conn
            .query_row(SQL_COUNT_ALL, [], |row| row.get::<_, i64>(0))
            .map_err(|e| CoreError::Storage(format!("count_all: {e}")))
    }

    /// Soft-delete a bookmark by id. The row remains with `archived=1`
    /// so re-inserting the same canonical URL is possible without
    /// violating the unique index.
    pub fn delete(&mut self, id: &BookmarkId) -> Result<(), CoreError> {
        let affected = self
            .conn
            .execute(SQL_ARCHIVE_BY_ID, params![id.0])
            .map_err(|e| CoreError::Storage(format!("delete: {e}")))?;
        if affected == 0 {
            return Err(CoreError::Storage(format!(
                "delete: no active row with id {}",
                id.0
            )));
        }
        Ok(())
    }

    /// Replace the tag set for a bookmark. Tags are normalized through
    /// [`Tag::new`] before insertion; duplicates collapse naturally
    /// (composite PK on `(bookmark_id, tag)`).
    pub fn set_tags(&mut self, id: &BookmarkId, tags: &[String]) -> Result<(), CoreError> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| CoreError::Storage(format!("begin tags tx: {e}")))?;
        tx.execute(SQL_DELETE_TAGS_FOR, params![id.0])
            .map_err(|e| CoreError::Storage(format!("delete tags: {e}")))?;
        for raw in tags {
            if let Some(tag) = Tag::new(raw) {
                tx.execute(
                    SQL_INSERT_TAG,
                    params![id.0, tag.0],
                )
                .map_err(|e| CoreError::Storage(format!("insert tag: {e}")))?;
            }
        }
        tx.commit()
            .map_err(|e| CoreError::Storage(format!("commit tags tx: {e}")))?;
        Ok(())
    }

    /// Read all tags attached to a bookmark, sorted alphabetically.
    pub fn tags_for(&self, id: &BookmarkId) -> Result<Vec<String>, CoreError> {
        let mut stmt = self
            .conn
            .prepare_cached(SQL_SELECT_TAGS_FOR)
            .map_err(|e| CoreError::Storage(format!("prepare tags_for: {e}")))?;
        let rows = stmt
            .query_map(params![id.0], |row| row.get::<_, String>(0))
            .map_err(|e| CoreError::Storage(format!("tags_for query: {e}")))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| CoreError::Storage(format!("tag row: {e}")))?);
        }
        Ok(out)
    }

    // ---- internal helpers ----

    fn insert_new(
        &self,
        id: &BookmarkId,
        b: &Bookmark,
        now: i64,
    ) -> Result<(), CoreError> {
        let added_at = if b.created_at.timestamp() > 0 {
            b.created_at.timestamp()
        } else {
            now
        };
        let last_seen = if b.updated_at.timestamp() > 0 {
            b.updated_at.timestamp()
        } else {
            now
        };
        let raw_str = b
            .source
            .raw
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        self.conn
            .execute(
                SQL_INSERT_BOOKMARK,
                params![
                    id.0,
                    b.original_url,
                    b.canonical_url,
                    b.title,
                    b.description,
                    b.collection,
                    b.source.kind.as_cli_str(),
                    Option::<String>::None, // source_id — unused in Fase 1
                    b.source.external_id,
                    added_at,
                    last_seen,
                    raw_str,
                    if b.archived { 1i64 } else { 0i64 },
                ],
            )
            .map_err(|e| CoreError::Storage(format!("insert bookmark: {e}")))?;
        Ok(())
    }

    fn update_existing(
        &self,
        id: &BookmarkId,
        b: &Bookmark,
        _now: i64,
    ) -> Result<(), CoreError> {
        // Preserve the caller-supplied `updated_at` so import-time
        // provenance (e.g. Chrome's last-visit timestamp) round-trips.
        // Fall back to "now" only when the caller left it at epoch 0.
        let last_seen = if b.updated_at.timestamp() > 0 {
            b.updated_at.timestamp()
        } else {
            _now
        };
        let raw_str = b
            .source
            .raw
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        self.conn
            .execute(
                SQL_UPDATE_BOOKMARK,
                params![
                    b.original_url,
                    b.title,
                    b.description,
                    b.collection,
                    b.source.kind.as_cli_str(),
                    b.source.external_id,
                    last_seen,
                    raw_str,
                    id.0,
                ],
            )
            .map_err(|e| CoreError::Storage(format!("update bookmark: {e}")))?;
        Ok(())
    }
}

// --- SQL constants -----------------------------------------------------

const SQL_LIST_ACTIVE: &str = "\
    SELECT id, original_url, canonical_url, title, description, collection, \
     source_kind, source_id, external_id, added_at, last_seen_at, raw, archived \
    FROM bookmarks WHERE archived = 0 \
    ORDER BY last_seen_at DESC, id ASC LIMIT ?1 OFFSET ?2";

const SQL_COUNT_ACTIVE: &str = "SELECT COUNT(*) FROM bookmarks WHERE archived = 0";
const SQL_COUNT_ALL: &str = "SELECT COUNT(*) FROM bookmarks";

const SQL_SELECT_TAGS_FOR: &str =
    "SELECT tag FROM tags WHERE bookmark_id = ?1 ORDER BY tag ASC";
const SQL_DELETE_TAGS_FOR: &str = "DELETE FROM tags WHERE bookmark_id = ?1";
const SQL_INSERT_TAG: &str =
    "INSERT OR IGNORE INTO tags (bookmark_id, tag) VALUES (?1, ?2)";

const SQL_ARCHIVE_BY_ID: &str =
    "UPDATE bookmarks SET archived = 1 WHERE id = ?1 AND archived = 0";

const SQL_INSERT_BOOKMARK: &str = "\
    INSERT INTO bookmarks \
    (id, original_url, canonical_url, title, description, collection, \
     source_kind, source_id, external_id, added_at, last_seen_at, raw, archived) \
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)";

const SQL_UPDATE_BOOKMARK: &str = "\
    UPDATE bookmarks SET \
        original_url = ?1, title = ?2, description = ?3, collection = ?4, \
        source_kind = ?5, external_id = ?6, last_seen_at = ?7, raw = ?8 \
    WHERE id = ?9";

fn row_to_bookmark(row: &Row<'_>) -> rusqlite::Result<Bookmark> {
    let id: String = row.get(0)?;
    let original_url: String = row.get(1)?;
    let canonical_url: String = row.get(2)?;
    let title: String = row.get(3)?;
    let description: Option<String> = row.get(4)?;
    let collection: Option<String> = row.get(5)?;
    let source_kind_str: String = row.get(6)?;
    let _source_id: Option<String> = row.get(7)?;
    let external_id: Option<String> = row.get(8)?;
    let added_at: i64 = row.get(9)?;
    let last_seen_at: i64 = row.get(10)?;
    let raw_str: Option<String> = row.get(11)?;
    let archived: i64 = row.get(12)?;

    let kind = SourceKind::from_cli_str(&source_kind_str).unwrap_or(SourceKind::Manual);
    let raw = raw_str.and_then(|s| serde_json::from_str(&s).ok());
    let source = SourceRef {
        kind,
        external_id,
        imported_at: Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now),
        raw,
    };

    Ok(Bookmark {
        id: BookmarkId(id),
        original_url,
        canonical_url,
        title,
        description,
        tags: Vec::new(), // filled by caller via tags_for()
        collection,
        created_at: epoch_to_utc(added_at),
        updated_at: epoch_to_utc(last_seen_at),
        source,
        content_type: None, // not persisted in Fase 2 schema
        archived: archived != 0,
    })
}

fn epoch_to_utc(secs: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now)
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
    use crate::model::SourceKind;
    use chrono::TimeZone;

    fn mk(canonical: &str, title: &str, source: SourceKind, secs: i64) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: format!("https://example.com/{title}"),
            canonical_url: canonical.into(),
            title: title.into(),
            description: None,
            tags: vec!["rust".into(), "cli".into()],
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

    #[test]
    fn insert_new_returns_id() {
        let mut s = open_in_memory().unwrap();
        let b = mk("https://example.com/a", "A", SourceKind::Chromium, 1_000);
        let id = s.upsert(&b).unwrap();
        assert_eq!(s.count().unwrap(), 1);
        let fetched = s.by_canonical("https://example.com/a").unwrap().unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.title, "A");
    }

    #[test]
    fn by_canonical_returns_none_when_missing() {
        let s = open_in_memory().unwrap();
        assert!(s
            .by_canonical("https://example.com/missing")
            .unwrap()
            .is_none());
    }

    #[test]
    fn list_pagination_orders_by_last_seen_then_id() {
        let mut s = open_in_memory().unwrap();
        let b1 = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
        let b2 = mk("https://example.com/b", "B", SourceKind::Chromium, 200);
        let b3 = mk("https://example.com/c", "C", SourceKind::Chromium, 200);
        s.upsert(&b1).unwrap();
        s.upsert(&b2).unwrap();
        s.upsert(&b3).unwrap();

        let all = s.list(10, 0).unwrap();
        assert_eq!(all.len(), 3);
        let last_seens: Vec<i64> = all.iter().map(|b| b.updated_at.timestamp()).collect();
        assert_eq!(last_seens, vec![200, 200, 100]);

        let page1 = s.list(2, 0).unwrap();
        let page2 = s.list(2, 2).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page2.len(), 1);
    }

    #[test]
    fn count_reflects_upserts_and_deletes() {
        let mut s = open_in_memory().unwrap();
        assert_eq!(s.count().unwrap(), 0);
        let b1 = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
        let b2 = mk("https://example.com/b", "B", SourceKind::Chromium, 200);
        let id1 = s.upsert(&b1).unwrap();
        let _id2 = s.upsert(&b2).unwrap();
        assert_eq!(s.count().unwrap(), 2);
        s.delete(&id1).unwrap();
        assert_eq!(s.count().unwrap(), 1);
        assert_eq!(s.count_all().unwrap(), 2);
    }

    #[test]
    fn tags_set_and_replace() {
        let mut s = open_in_memory().unwrap();
        let mut b = mk("https://example.com/a", "A", SourceKind::Chromium, 100);
        b.tags = vec!["Rust".into(), "  CLI  ".into(), "".into(), "Rust".into()];
        let id = s.upsert(&b).unwrap();
        let tags = s.tags_for(&id).unwrap();
        assert_eq!(tags, vec!["cli".to_string(), "rust".to_string()]);

        s.set_tags(&id, &["new".into(), "another".into()]).unwrap();
        let tags = s.tags_for(&id).unwrap();
        assert_eq!(tags, vec!["another".to_string(), "new".to_string()]);

        s.set_tags(&id, &[]).unwrap();
        let tags = s.tags_for(&id).unwrap();
        assert!(tags.is_empty());
    }
}