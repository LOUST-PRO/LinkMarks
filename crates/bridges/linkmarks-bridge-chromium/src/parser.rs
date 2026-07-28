//! Parser for the Chromium `Bookmarks` JSON format.
//!
//! File shape (top-level):
//! ```json
//! {
//!   "roots": {
//!     "bookmark_bar": { "type": "folder", "name": "Bookmarks bar", "children": [...] },
//!     "other":       { "type": "folder", "name": "Other bookmarks", "children": [...] },
//!     "synced":      { "type": "folder", "name": "Mobile bookmarks", "children": [...] }
//!   }
//! }
//! ```
//!
//! Each child is either a `folder` (with `children`) or a `url`
//! (with `url` field). Optional fields: `name`, `date_added`,
//! `date_last_used`, `id`, `guid`.
//!
//! The parser is **tolerant**: missing `name`, missing `url` on a
//! url-type node, missing `date_added` — all are recorded as
//! `ParseError::Partial` and the rest of the file is parsed.

use chrono::{DateTime, TimeZone, Utc};
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef, Tag};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use thiserror::Error;

/// Errors from parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse failure.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    /// Top-level shape doesn't match expected.
    #[error("invalid root: {0}")]
    InvalidRoot(String),
    /// Per-element failure (non-fatal — see `parse_file_partial`).
    #[error("partial failure at {element}: {reason}")]
    Partial {
        /// URL or path of the offending element.
        element: String,
        /// Human-readable reason.
        reason: String,
    },
}

/// Top-level deserialized shape.
#[derive(Debug, Deserialize)]
pub struct ChromiumBookmarks {
    /// Map of root containers (`bookmark_bar`, `other`, `synced`).
    pub roots: Roots,
}

/// Container of root folders.
#[derive(Debug, Deserialize)]
pub struct Roots {
    /// Main bookmarks bar.
    pub bookmark_bar: BookmarkNode,
    /// Other bookmarks (uncategorized).
    pub other: BookmarkNode,
    /// Synced bookmarks (mobile etc.); absent in some browsers.
    #[serde(default)]
    pub synced: Option<BookmarkNode>,
}

/// Recursive node.
#[derive(Debug, Deserialize, Clone)]
pub struct BookmarkNode {
    /// `"folder"` or `"url"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// URL (only for `type == "url"`).
    #[serde(default)]
    pub url: Option<String>,
    /// Children (only for `type == "folder"`).
    #[serde(default)]
    pub children: Vec<BookmarkNode>,
    /// Chromium timestamp (microseconds since Windows epoch
    /// 1601-01-01). Optional.
    #[serde(default)]
    pub date_added: Option<String>,
    /// Last-used timestamp, same encoding.
    #[serde(default)]
    pub date_last_used: Option<String>,
}

/// Read a file and parse it into a `ChromiumBookmarks` struct.
pub fn parse_file(path: &Path) -> Result<ChromiumBookmarks, ParseError> {
    let bytes = std::fs::read(path)?;
    let parsed: ChromiumBookmarks = serde_json::from_slice(&bytes)?;
    Ok(parsed)
}

/// Walk the parsed tree and yield normalized `Bookmark` records.
///
/// `collection_prefix` is the folder path leading to this node
/// (e.g., `"Work/Research"`); it becomes the bookmark's `collection`.
pub fn flatten(roots: &ChromiumBookmarks) -> (Vec<Bookmark>, Vec<ParseError>) {
    let mut bookmarks = Vec::new();
    let mut errors = Vec::new();

    flatten_node(&roots.roots.bookmark_bar, "", &mut bookmarks, &mut errors);
    flatten_node(&roots.roots.other, "", &mut bookmarks, &mut errors);
    if let Some(synced) = &roots.roots.synced {
        flatten_node(synced, "", &mut bookmarks, &mut errors);
    }

    (bookmarks, errors)
}

fn flatten_node(
    node: &BookmarkNode,
    collection_prefix: &str,
    out: &mut Vec<Bookmark>,
    errors: &mut Vec<ParseError>,
) {
    match node.kind.as_str() {
        "url" => match build_bookmark(node, collection_prefix) {
            Ok(b) => out.push(b),
            Err(e) => errors.push(e),
        },
        "folder" => {
            let next_collection = if collection_prefix.is_empty() {
                node.name.clone()
            } else {
                format!("{collection_prefix}/{}", node.name)
            };
            for child in &node.children {
                flatten_node(child, &next_collection, out, errors);
            }
        }
        other => {
            errors.push(ParseError::Partial {
                element: format!("node({other})"),
                reason: format!("unknown node type '{other}'"),
            });
        }
    }
}

fn build_bookmark(node: &BookmarkNode, collection: &str) -> Result<Bookmark, ParseError> {
    let url = node.url.as_deref().ok_or_else(|| ParseError::Partial {
        element: node.name.clone(),
        reason: "url node missing 'url' field".to_string(),
    })?;
    if url.is_empty() {
        return Err(ParseError::Partial {
            element: node.name.clone(),
            reason: "url node has empty 'url' field".to_string(),
        });
    }

    let canonical = linkmarks_core::canonicalize(url).map_err(|e| ParseError::Partial {
        element: url.to_string(),
        reason: format!("canonicalize failed: {e}"),
    })?;

    let created_at = parse_chromium_timestamp(node.date_added.as_deref()).unwrap_or_else(Utc::now);
    let updated_at = parse_chromium_timestamp(node.date_last_used.as_deref()).unwrap_or(created_at);

    let tags_set: BTreeSet<String> = BTreeSet::new();
    let _ = tags_set;

    Ok(Bookmark {
        id: BookmarkId::generate(),
        original_url: url.to_string(),
        canonical_url: canonical,
        title: node.name.trim().to_string(),
        description: None,
        tags: Vec::new(),
        collection: if collection.is_empty() {
            None
        } else {
            Some(collection.to_string())
        },
        created_at,
        updated_at,
        source: SourceRef {
            kind: SourceKind::Chromium,
            external_id: None,
            imported_at: Utc::now(),
            raw: None,
        },
        content_type: None,
        archived: false,
    })
}

/// Chromium timestamps are microseconds since 1601-01-01 UTC (Windows
/// FILETIME epoch). Returns `None` for missing or unparseable input.
fn parse_chromium_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = raw?;
    let micros: i64 = raw.parse().ok()?;
    // Windows epoch is 1601-01-01. Unix epoch (1970-01-01) is
    // 11644473600 seconds = 11_644_473_600_000_000 microseconds later.
    let unix_micros = micros.checked_sub(11_644_473_600_000_000)?;
    let secs = unix_micros.div_euclid(1_000_000);
    let nsec = (unix_micros.rem_euclid(1_000_000) * 1000) as u32;
    Utc.timestamp_opt(secs, nsec).single()
}

/// Helper: parse + flatten in one call. Returns the bookmarks plus
/// any per-element errors. The caller decides how to surface errors.
pub fn parse_and_flatten(path: &Path) -> Result<(Vec<Bookmark>, Vec<ParseError>), ParseError> {
    let parsed = parse_file(path)?;
    Ok(flatten(&parsed))
}

// Suppress unused warnings for the future-facing Tag import.
#[allow(dead_code)]
fn _tag_typecheck(t: Tag) -> String {
    t.0
}

// Suppress unused CoreError import (re-exported through public API).
#[allow(dead_code)]
fn _ce_typecheck(e: CoreError) -> String {
    format!("{e}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_url_node() {
        let json = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "Bookmarks bar",
                    "children": [
                        {
                            "type": "url",
                            "name": "Example",
                            "url": "https://example.com/"
                        }
                    ]
                },
                "other": {"type": "folder", "name": "Other", "children": []}
            }
        }"#;
        let parsed: ChromiumBookmarks = serde_json::from_str(json).unwrap();
        let (bookmarks, errors) = flatten(&parsed);
        assert!(errors.is_empty(), "errors: {errors:?}");
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].title, "Example");
        assert_eq!(bookmarks[0].original_url, "https://example.com/");
        assert_eq!(bookmarks[0].canonical_url, "https://example.com/");
    }

    #[test]
    fn walks_nested_folders() {
        let json = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "Bookmarks bar",
                    "children": [
                        {
                            "type": "folder",
                            "name": "Work",
                            "children": [
                                {
                                    "type": "url",
                                    "name": "A",
                                    "url": "https://example.com/a"
                                },
                                {
                                    "type": "folder",
                                    "name": "Research",
                                    "children": [
                                        {
                                            "type": "url",
                                            "name": "B",
                                            "url": "https://example.com/b"
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                },
                "other": {"type": "folder", "name": "Other", "children": []}
            }
        }"#;
        let parsed: ChromiumBookmarks = serde_json::from_str(json).unwrap();
        let (bookmarks, errors) = flatten(&parsed);
        assert!(errors.is_empty(), "errors: {errors:?}");
        assert_eq!(bookmarks.len(), 2);
        let collections: Vec<&str> = bookmarks
            .iter()
            .map(|b| b.collection.as_deref().unwrap_or(""))
            .collect();
        // Top-level folders prepend the root folder name (Bookmarks bar).
        assert!(
            collections.contains(&"Bookmarks bar/Work"),
            "collections were: {collections:?}"
        );
        assert!(collections.contains(&"Bookmarks bar/Work/Research"));
    }

    #[test]
    fn reports_partial_failure_on_missing_url() {
        let json = r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "Bookmarks bar",
                    "children": [
                        {"type": "url", "name": "no-url"}
                    ]
                },
                "other": {"type": "folder", "name": "Other", "children": []}
            }
        }"#;
        let parsed: ChromiumBookmarks = serde_json::from_str(json).unwrap();
        let (bookmarks, errors) = flatten(&parsed);
        assert_eq!(bookmarks.len(), 0);
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn chromium_timestamp_decodes() {
        // 2020-02-14T00:00:00Z in Chromium microseconds.
        // 13226064000000000 = (2020-02-14 - 1601-01-01) in micros.
        let ts = parse_chromium_timestamp(Some("13226064000000000")).unwrap();
        assert_eq!(ts.timestamp(), 1581590400);
    }
}
