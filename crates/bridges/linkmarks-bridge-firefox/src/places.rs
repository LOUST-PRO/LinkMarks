//! Read-only parser for Firefox `places.sqlite`.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

use chrono::{DateTime, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use rusqlite::{Connection, ErrorCode, OpenFlags, Row};

use crate::errors::BridgeError;

/// Number of retry attempts when `places.sqlite` is contended by
/// another process (typically a running Firefox instance writing to it).
const RETRY_MAX_ATTEMPTS: u32 = 3;
/// Backoff base in milliseconds — multiplied by the attempt index.
const RETRY_BACKOFF_MS: u64 = 100;

/// URL schemes that should be skipped even when Firefox emits them as
/// bookmark entries. These are internal/virtual addresses with no
/// canonical external target.
const INTERNAL_URL_PREFIXES: &[&str] = &["place:", "about:", "javascript:", "chrome:", "data:"];

/// Raw shape of a row read from `moz_places` + `moz_bookmarks`. Only
/// the columns we actually use are decoded; unknown columns (e.g. the
/// optional `description` field on `moz_places`) are tolerated by the
/// caller via the `has_description` probe above.
#[derive(Debug)]
struct PlaceRow {
    id: i64,
    parent: i64,
    kind: i64,
    fk: Option<i64>,
    title: String,
    url: Option<String>,
    description: Option<String>,
    visited: Option<i64>,
    last_modified: Option<i64>,
}

/// Returns `true` for SQLite errors that signal transient contention
/// and should be retried with backoff.
fn is_busy_or_locked(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == ErrorCode::DatabaseBusy || err.code == ErrorCode::DatabaseLocked
    )
}

/// Returns `true` for URL schemes Firefox stores as bookmarks but that
/// point at internal addresses with no canonical external target.
///
/// Comparison is case-insensitive on the URI scheme (the part before
/// the first `:`) so URLs like `ABOUT:HOME` or `JavaScript:void(0)`
/// emitted by Firefox or third-party extensions are filtered the same
/// as their lowercase canonical forms.
fn is_internal_url(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once(':') else {
        return false;
    };
    INTERNAL_URL_PREFIXES.iter().any(|prefix| {
        // Each entry ends with `:` — compare scheme case-insensitively.
        prefix
            .strip_suffix(':')
            .is_some_and(|s| s.eq_ignore_ascii_case(scheme))
    })
}

/// Open `places.sqlite` in read-only mode and apply the busy-timeout
/// pragma. Does NOT retry on its own — the retry loop wraps the entire
/// read flow (`open + prepare + query_map + iteration`) in
/// [`parse_places`] so any stage that hits contention is covered.
fn open_connection(path: &Path) -> Result<Connection, BridgeError> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(BridgeError::SqliteOpen)?;
    conn.busy_timeout(Duration::from_millis(
        linkmarks_core::storage::BUSY_TIMEOUT_MS as u64,
    ))
    .map_err(BridgeError::SqliteQuery)?;
    Ok(conn)
}

/// Read flow used inside the retry loop. All SQL access stages that
/// can hit `SQLITE_BUSY`/`SQLITE_LOCKED` (open, prepare, query_map,
/// row iteration) live here so the caller can re-run the whole flow
/// when any one stage fails under contention.
fn read_places(path: &Path) -> Result<Vec<Bookmark>, BridgeError> {
    let connection = open_connection(path)?;
    let has_description = connection
        .prepare("PRAGMA table_info(moz_places)")
        .map_err(BridgeError::SqliteQuery)?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(BridgeError::SqliteQuery)?
        .filter_map(Result::ok)
        .any(|name| name == "description");
    let description_sql = if has_description {
        "p.description"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT b.id, b.parent, b.type, b.fk, b.title, p.url, {description_sql}, p.last_visit_date, b.lastModified
         FROM moz_bookmarks b LEFT JOIN moz_places p ON b.fk = p.id
         ORDER BY b.parent, b.position, b.id"
    );
    let mut statement = connection.prepare(&sql).map_err(BridgeError::SqliteQuery)?;
    let rows = statement
        .query_map([], row_to_place)
        .map_err(BridgeError::SqliteQuery)?;
    let mut by_id = HashMap::new();
    for row in rows {
        let row = row.map_err(BridgeError::SqliteQuery)?;
        by_id.insert(row.id, row);
    }

    let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
    for row in by_id.values() {
        children.entry(row.parent).or_default().push(row.id);
    }
    let mut output = Vec::new();
    let mut emitted = HashSet::new();
    for root in [1_i64, 2, 3] {
        if by_id.contains_key(&root) {
            walk(root, &[], &by_id, &children, &mut emitted, &mut output);
        }
    }
    Ok(output)
}

/// Parse a Firefox profile database without ever opening it for writing.
///
/// Retries the complete read flow (open + prepare + query_map +
/// iteration) up to [`RETRY_MAX_ATTEMPTS`] times when SQLite reports
/// transient contention (`SQLITE_BUSY` / `SQLITE_LOCKED`). This covers
/// any stage that Firefox's own writes can block — not only the open
/// itself. Exhaustion surfaces as
/// [`BridgeError::DatabaseLocked`] with the attempt count and the last
/// error string for diagnostics.
pub fn parse_places(path: &Path) -> Result<Vec<Bookmark>, BridgeError> {
    let mut attempts: u32 = 0;
    loop {
        match read_places(path) {
            Ok(bookmarks) => return Ok(bookmarks),
            Err(error) if is_busy_or_locked_error(&error) && attempts < RETRY_MAX_ATTEMPTS => {
                attempts += 1;
                sleep(Duration::from_millis(RETRY_BACKOFF_MS * attempts as u64));
            }
            Err(error) if is_busy_or_locked_error(&error) => {
                return Err(BridgeError::DatabaseLocked {
                    attempts,
                    last_error: busy_or_locked_message(&error).unwrap_or_default(),
                });
            }
            Err(other) => return Err(other),
        }
    }
}

/// Returns `true` when the [`BridgeError`] wraps a rusqlite
/// `SQLITE_BUSY` / `SQLITE_LOCKED` failure.
fn is_busy_or_locked_error(error: &BridgeError) -> bool {
    match error {
        BridgeError::SqliteOpen(inner) | BridgeError::SqliteQuery(inner) => {
            is_busy_or_locked(inner)
        }
        BridgeError::DatabaseLocked { .. } => false,
        _ => false,
    }
}

/// Extract the string form of the inner rusqlite error if the
/// [`BridgeError`] wraps one of the contention variants.
fn busy_or_locked_message(error: &BridgeError) -> Option<String> {
    match error {
        BridgeError::SqliteOpen(inner) | BridgeError::SqliteQuery(inner) => Some(inner.to_string()),
        _ => None,
    }
}

/// Decode a single SQL row produced by the `moz_bookmarks LEFT JOIN
/// moz_places` query into a [`PlaceRow`]. Column indices match the
/// SELECT projection in [`read_places`].
fn row_to_place(row: &Row<'_>) -> rusqlite::Result<PlaceRow> {
    Ok(PlaceRow {
        id: row.get(0)?,
        parent: row.get(1)?,
        kind: row.get(2)?,
        fk: row.get(3)?,
        title: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        url: row.get(5)?,
        description: row.get(6)?,
        visited: row.get(7)?,
        last_modified: row.get(8)?,
    })
}

/// Recursively walk the `moz_bookmarks` tree starting at `id`,
/// emitting one [`Bookmark`] per regular bookmark entry (type=1) and
/// tracking folder ancestors for the `collection` and `tags` fields.
///
/// `ancestors` is the chain of folder names from the closest canonical
/// root (Bookmarks Menu / Toolbar / Other) down to — but not including —
/// the current folder. Mozilla `type` semantics: 1 = bookmark, 2 =
/// folder, 3 = separator (and any other value is skipped). Cycles are
/// prevented via the `emitted` set keyed by row id.
fn walk(
    id: i64,
    ancestors: &[String],
    rows: &HashMap<i64, PlaceRow>,
    children: &HashMap<i64, Vec<i64>>,
    emitted: &mut HashSet<i64>,
    output: &mut Vec<Bookmark>,
) {
    let Some(folder) = rows.get(&id) else {
        return;
    };

    // Mozilla `moz_bookmarks.type` semantics:
    //   1 = regular bookmark (emit)
    //   2 = folder          (push name into ancestors, recurse)
    //   3 = separator       (skip)
    // Anything else is treated as a separator (skip).
    match folder.kind {
        1 => {
            if !emitted.insert(id) {
                return;
            }
            let Some(url) = folder.url.as_deref().filter(|url| !url.is_empty()) else {
                return;
            };
            if is_internal_url(url) {
                return;
            }
            let created_at = folder
                .visited
                .and_then(DateTime::from_timestamp_micros)
                .unwrap_or_default();
            let updated_at = folder
                .last_modified
                .and_then(DateTime::from_timestamp_micros)
                .unwrap_or(created_at);
            // Fall back to the raw URL when canonicalization fails so the
            // caller (`import.rs::canonicalize_bookmarks`) can apply its
            // own config and decide whether to keep or drop.
            let canonical_url =
                linkmarks_core::canonicalize(url).unwrap_or_else(|_| url.to_string());
            output.push(Bookmark {
                id: BookmarkId::generate(),
                original_url: url.to_string(),
                canonical_url,
                title: folder.title.trim().to_string(),
                description: folder.description.clone().filter(|v| !v.is_empty()),
                tags: ancestors
                    .iter()
                    .map(|name| format!("#folder/{}", slug(name)))
                    .collect(),
                collection: (!ancestors.is_empty()).then(|| ancestors.join("/")),
                created_at,
                updated_at,
                source: SourceRef {
                    kind: SourceKind::Firefox,
                    external_id: Some(
                        folder
                            .fk
                            .map_or_else(|| folder.id.to_string(), |fk| fk.to_string()),
                    ),
                    imported_at: Utc::now(),
                    raw: None,
                },
                content_type: None,
                archived: false,
            });
        }
        2 => {
            let root_name = match id {
                1 => "Bookmarks Menu",
                2 => "Bookmarks Toolbar",
                3 => "Other Bookmarks",
                _ => folder.title.trim(),
            };
            let mut next = ancestors.to_vec();
            if !root_name.is_empty() && !root_name.to_ascii_lowercase().starts_with("tag:") {
                next.push(root_name.to_string());
            }
            if let Some(ids) = children.get(&id) {
                for child in ids {
                    walk(*child, &next, rows, children, emitted, output);
                }
            }
        }
        _ => {
            // Separators and unknown types: skip.
        }
    }
}

/// Slugify a folder name for use inside a `#folder/<slug>` tag.
/// Non-alphanumeric runs collapse to single `-` and leading/trailing
/// dashes are trimmed so the tag is safe to embed in a tag tree.
fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut sep = false;
    for c in value.trim().to_lowercase().chars() {
        if c.is_alphanumeric() {
            out.push(c);
            sep = false;
        } else if !out.is_empty() && !sep {
            out.push('-');
            sep = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}
