//! Read-only parser for Firefox `places.sqlite`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use rusqlite::{Connection, OpenFlags, Row};

use crate::errors::BridgeError;

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
}

/// Parse a Firefox profile database without ever opening it for writing.
pub fn parse_places(path: &Path) -> Result<Vec<Bookmark>, BridgeError> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(BridgeError::SqliteOpen)?;
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
        "SELECT b.id, b.parent, b.type, b.fk, b.title, p.url, {description_sql}, p.last_visit_date
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
    })
}

fn walk(
    id: i64,
    ancestors: &[String],
    rows: &HashMap<i64, PlaceRow>,
    children: &HashMap<i64, Vec<i64>>,
    emitted: &mut HashSet<i64>,
    output: &mut Vec<Bookmark>,
) {
    let Some(folder) = rows.get(&id) else { return };
    if folder.kind == 2 || folder.kind == 0 {
        if folder.kind == 0 && emitted.insert(id) {
            if let Some(url) = folder.url.as_deref().filter(|url| !url.is_empty()) {
                if let Ok(canonical_url) = linkmarks_core::canonicalize(url) {
                    let timestamp = folder
                        .visited
                        .and_then(DateTime::from_timestamp_micros)
                        .unwrap_or_default();
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
                        created_at: timestamp,
                        updated_at: timestamp,
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
            }
        }
        return;
    }
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
