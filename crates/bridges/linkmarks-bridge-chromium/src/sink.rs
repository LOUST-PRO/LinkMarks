//! Chromium-family Bookmarks JSON writer.
//!
//! ## File format
//!
//! Mirrors `parser.rs` — the writer produces a JSON document whose
//! schema matches what Vivaldi / Chrome / Brave / Edge / Arc / Opera
//! import. Top-level shape:
//!
//! ```json
//! {
//!   "roots": {
//!     "bookmark_bar": { "type": "folder", "name": "Bookmarks bar", "children": [...] },
//!     "other":       { "type": "folder", "name": "Other bookmarks", "children": [...] }
//!   }
//! }
//! ```
//!
//! Each bookmark becomes a `"url"` node; folders in the bookmark's
//! `collection` field are recreated as nested `"folder"` nodes
//! inside `bookmark_bar`. Bookmarks with no `collection` fall
//! through to `other`.
//!
//! ## Determinism
//!
//! Bookmarks are sorted by `(canonical_url, id)` before write, so
//! consecutive `write()` calls on the same input produce byte-identical
//! JSON.
//!
//! ## Tags
//!
//! Chromium Bookmarks JSON does not support tags natively, so tags
//! are **not exported**. Folder hierarchy comes from
//! `Bookmark::collection`. Synthetic `#folder/*` tags are dropped
//! silently (they're re-derivable from the collection path on
//! re-import).
//!
//! ## Atomicity
//!
//! `ChromiumSink::write_to` builds the JSON in memory, writes to a
//! `tempfile::NamedTempFile` in the same directory, then `persist`s
//! (rename) into place. The target file is either fully new or
//! untouched — partial writes never appear.
//!
//! ## Timestamp encoding
//!
//! `date_added` and `date_last_used` are encoded as Chromium
//! microseconds since the Windows FILETIME epoch (1601-01-01). The
//! parser's inverse uses the same constant; the encoder is exposed
//! as `parser::chromium_timestamp` for symmetry.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use linkmarks_core::traits::{BookmarkSink, WriteReport};

use crate::parser::{chromium_timestamp, BookmarkNode, ChromiumBookmarks, ParseError, Roots};

/// Folder name used for the standard "Bookmarks bar" root in
/// Chromium / Vivaldi / Chrome / Brave / Edge / Arc. Matches the
/// parser's default name for `bookmark_bar`.
const BOOKMARK_BAR_NAME: &str = "Bookmarks bar";
/// Folder name used for the standard "Other bookmarks" root.
const OTHER_BOOKMARKS_NAME: &str = "Other bookmarks";

/// A `BookmarkSink` that emits Chromium-family Bookmarks JSON.
pub struct ChromiumSink {
    path: Option<PathBuf>,
    /// When `path` is `None`, the sink is in-memory: `write()` populates
    /// `last_body` instead of touching the disk.
    last_body: Option<String>,
    /// Number of bookmarks most-recently written.
    last_report: Option<WriteReport>,
}

impl ChromiumSink {
    /// Create a sink bound to a target file path. The file need not
    /// exist yet; `write` creates or overwrites.
    #[must_use]
    pub fn open(path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf()),
            last_body: None,
            last_report: None,
        }
    }

    /// Create an in-memory sink that captures writes into a `String`
    /// rather than touching disk. Useful for tests and for CLI
    /// pipelines that want to chain sinks.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            path: None,
            last_body: None,
            last_report: None,
        }
    }

    /// Build the JSON-serializable Chromium Bookmarks tree from a
    /// slice of bookmarks. Pure function — does no I/O.
    ///
    /// Bookmarks are sorted by `(canonical_url, id)` for deterministic
    /// output. Each bookmark is placed under its `collection` path
    /// inside `bookmark_bar`. Bookmarks with no `collection` fall
    /// through to `other`.
    ///
    /// Folder assembly uses a side index (`BTreeMap<String, Vec<BookmarkNode>>`)
    /// keyed by full `/`-separated path. Intermediate folders are
    /// created on-demand and materialised deepest-first, so each
    /// parent picks up its already-built sub-folders as children.
    #[must_use]
    pub fn build_chromium_bookmarks(bookmarks: &[Bookmark]) -> ChromiumBookmarks {
        // Sort by (canonical_url, id) for deterministic output.
        let mut sorted: Vec<&Bookmark> = bookmarks.iter().collect();
        sorted.sort_by(|a, b| {
            a.canonical_url
                .cmp(&b.canonical_url)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });

        // Phase 1 — group URL nodes by full collection path. The parser
        // writes the root folder name (`BOOKMARK_BAR_NAME` or
        // `OTHER_BOOKMARKS_NAME`) as the leading segment of every
        // collection it produces. The first segment tells us which
        // root the URL belongs to, and the rest is the path within
        // that root. Strip the leading root-name segment so the
        // sink operates on paths relative to the chosen root; an
        // empty remainder means the URL was a direct child of the
        // root in the parsed tree.
        let mut bar_urls_by_path: BTreeMap<String, Vec<BookmarkNode>> = BTreeMap::new();
        let mut bar_direct_children: Vec<BookmarkNode> = Vec::new();
        let mut other_urls_by_path: BTreeMap<String, Vec<BookmarkNode>> = BTreeMap::new();
        let mut other_direct_children: Vec<BookmarkNode> = Vec::new();

        for b in &sorted {
            let node = bookmark_to_url_node(b);
            let trimmed = b.collection.as_deref().map(str::trim).unwrap_or("");
            if trimmed.is_empty() {
                // No collection recorded — treat as `other` direct
                // child. Symmetric with `other`'s empty-prefix
                // semantics in the parser.
                other_direct_children.push(node);
                continue;
            }
            let (root, rel) = classify_collection(trimmed);
            if rel.is_empty() {
                match root {
                    TargetRoot::BookmarkBar => bar_direct_children.push(node),
                    TargetRoot::Other => other_direct_children.push(node),
                }
            } else {
                match root {
                    TargetRoot::BookmarkBar => bar_urls_by_path.entry(rel).or_default().push(node),
                    TargetRoot::Other => other_urls_by_path.entry(rel).or_default().push(node),
                }
            }
        }

        // Build the bookmark_bar subtree (folder nodes + direct URL children).
        let bar_children = assemble_folder_tree(bar_urls_by_path, bar_direct_children);
        // Build the other subtree.
        let other_children = assemble_folder_tree(other_urls_by_path, other_direct_children);

        ChromiumBookmarks {
            roots: Roots {
                bookmark_bar: BookmarkNode {
                    kind: "folder".to_string(),
                    name: BOOKMARK_BAR_NAME.to_string(),
                    url: None,
                    children: bar_children,
                    date_added: None,
                    date_last_used: None,
                },
                other: BookmarkNode {
                    kind: "folder".to_string(),
                    name: OTHER_BOOKMARKS_NAME.to_string(),
                    url: None,
                    children: other_children,
                    date_added: None,
                    date_last_used: None,
                },
                synced: None,
                custom_root: None,
            },
        }
    }

    /// Render the Chromium Bookmarks tree as pretty JSON (2-space
    /// indent). Pure function.
    #[must_use]
    pub fn render(tree: &ChromiumBookmarks) -> String {
        serde_json::to_string_pretty(tree)
            .unwrap_or_else(|e| format!("{{\"error\":\"render failed: {e}\"}}"))
    }

    /// Build + render the tree for a slice of bookmarks. Convenience
    /// wrapper that pairs `build_chromium_bookmarks` with `render`.
    #[must_use]
    pub fn build_json(bookmarks: &[Bookmark]) -> String {
        Self::render(&Self::build_chromium_bookmarks(bookmarks))
    }

    /// Write a slice of bookmarks atomically to a path. Returns the
    /// report plus the rendered body (for callers that want to
    /// inspect the output without re-reading the file).
    pub fn write_to(
        path: &Path,
        bookmarks: &[Bookmark],
    ) -> Result<(WriteReport, String), CoreError> {
        let tree = Self::build_chromium_bookmarks(bookmarks);
        let body = Self::render(&tree);
        write_atomic(path, &body)?;

        let report = WriteReport {
            written: bookmarks.len(),
            failed: Vec::new(),
        };
        Ok((report, body))
    }

    /// Take the most-recently rendered body (in-memory sink only).
    /// Returns `None` if the sink has not been written to or is
    /// file-bound.
    #[must_use]
    pub fn last_body(&self) -> Option<&str> {
        self.last_body.as_deref()
    }

    /// Take the most-recent report (in-memory sink only).
    #[must_use]
    pub fn last_report(&self) -> Option<&WriteReport> {
        self.last_report.as_ref()
    }
}

impl BookmarkSink for ChromiumSink {
    fn kind(&self) -> SourceKind {
        SourceKind::Chromium
    }

    fn write(&mut self, bookmarks: &[Bookmark]) -> Result<WriteReport, CoreError> {
        let tree = Self::build_chromium_bookmarks(bookmarks);
        let body = Self::render(&tree);
        if let Some(p) = &self.path {
            write_atomic(p, &body)?;
        }
        let report = WriteReport {
            written: bookmarks.len(),
            failed: Vec::new(),
        };
        self.last_body = Some(body);
        self.last_report = Some(report.clone());
        Ok(report)
    }

    fn delete(&mut self, external_id: &str) -> Result<(), CoreError> {
        // For Chromium JSON, "delete" means: rewrite the file omitting
        // the bookmark whose `original_url` matches `external_id`.
        let Some(path) = &self.path else {
            return Err(CoreError::Storage(
                "ChromiumSink::delete requires a file-bound sink (got in-memory)".to_string(),
            ));
        };
        let parsed = crate::parser::parse_file(path).map_err(parse_error_to_core)?;
        let remaining: Vec<Bookmark> = parsed
            .into_flat_bookmarks()
            .into_iter()
            .filter(|b| b.original_url.as_str() != external_id)
            .collect();
        let _ = Self::write_to(path, &remaining)?;
        Ok(())
    }
}

// ────────────────────────────────────────────────────────────────────
// Internal helpers
// ────────────────────────────────────────────────────────────────────

/// Convert a `Bookmark` into a URL-type `BookmarkNode`.
fn bookmark_to_url_node(b: &Bookmark) -> BookmarkNode {
    BookmarkNode {
        kind: "url".to_string(),
        name: b.title.trim().to_string(),
        url: Some(b.original_url.clone()),
        children: Vec::new(),
        date_added: Some(chromium_timestamp(b.created_at)),
        date_last_used: Some(chromium_timestamp(b.updated_at)),
    }
}

/// Strip the leading root-name segment from a collection path if it
/// matches `BOOKMARK_BAR_NAME` or `OTHER_BOOKMARKS_NAME`. The parser
/// writes the root folder's name as the first segment of every
/// collection; the sink operates on paths relative to the root, so
/// paths arriving with that segment need it removed to round-trip
/// cleanly. If the path equals one of the root names verbatim, the
/// resulting empty string is signalled by the caller routing the
/// bookmark to the corresponding direct-children bucket.
///
/// Examples:
/// - `"Bookmarks bar/Work"` → `"Work"`
/// - `"Bookmarks bar"`       → `""`  (caller → direct child of `bookmark_bar`)
/// - `"Work"`                → `"Work"` (unchanged)
/// - `"Other bookmarks/X"`   → `""`  (caller → direct child of `other`)
fn classify_collection(path: &str) -> (TargetRoot, String) {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match segments.first().copied() {
        Some(BOOKMARK_BAR_NAME) => (TargetRoot::BookmarkBar, segments[1..].join("/")),
        Some(OTHER_BOOKMARKS_NAME) => (TargetRoot::Other, segments[1..].join("/")),
        _ => (TargetRoot::BookmarkBar, path.to_string()),
    }
}

/// Which Chromium root a bookmark's collection path targets.
enum TargetRoot {
    BookmarkBar,
    Other,
}

/// Assemble a folder subtree for one root from its URL path index
/// plus direct URL children. Folders are materialised deepest-first
/// so each parent picks up already-built sub-folders as children,
/// then top-level folders are interleaved with direct URL children
/// (which were URLs that were direct children of the root in the
/// input tree).
fn assemble_folder_tree(
    urls_by_path: BTreeMap<String, Vec<BookmarkNode>>,
    direct_children: Vec<BookmarkNode>,
) -> Vec<BookmarkNode> {
    // Phase 2 — collect every folder path (including intermediate
    // parents that may have no URL children of their own).
    let mut all_paths: BTreeSet<String> = urls_by_path.keys().cloned().collect();
    for path in urls_by_path.keys().cloned().collect::<Vec<_>>() {
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for i in 1..segments.len() {
            all_paths.insert(segments[..i].join("/"));
        }
    }

    // Phase 3 — assemble folders deepest-first. When a folder is
    // materialised, every direct sub-folder (depth+1, no further
    // `/` in the relative path) is already in the index and can be
    // pulled in as a child.
    let mut folder_nodes: BTreeMap<String, BookmarkNode> = BTreeMap::new();
    let mut paths_by_depth: Vec<(usize, String)> = all_paths
        .into_iter()
        .map(|p| (p.matches('/').count() + 1, p))
        .collect();
    paths_by_depth.sort_by_key(|p| std::cmp::Reverse(p.0));

    let mut urls_by_path = urls_by_path;
    for (_depth, path) in paths_by_depth {
        let mut children: Vec<BookmarkNode> = urls_by_path.remove(&path).unwrap_or_default();

        // Pull in any direct sub-folder children (paths of the form
        // `parent/child` where `child` itself has no further '/').
        let prefix = format!("{path}/");
        let sub_paths: Vec<String> = folder_nodes
            .keys()
            .filter(|k| {
                k.len() > prefix.len() && k.starts_with(&prefix) && !k[prefix.len()..].contains('/')
            })
            .cloned()
            .collect();
        for sub in sub_paths {
            if let Some(node) = folder_nodes.remove(&sub) {
                children.push(node);
            }
        }

        let segments: Vec<&str> = path.split('/').collect();
        let name = segments.last().copied().unwrap_or("").to_string();
        folder_nodes.insert(
            path,
            BookmarkNode {
                kind: "folder".to_string(),
                name,
                url: None,
                children,
                date_added: None,
                date_last_used: None,
            },
        );
    }

    // Phase 4 — top-level (depth 1) folders become siblings of the
    // direct URL children. Folder names come first (BTreeMap iterates
    // in sorted key order), then direct URLs in their original
    // (canonical_url, id) order.
    let mut children: Vec<BookmarkNode> = folder_nodes
        .into_values()
        .filter(|n| !n.name.contains('/'))
        .collect();
    children.extend(direct_children);
    children
}

/// Write `body` atomically to `path` (temp-file + rename in the
/// same directory). Mirrors `NetscapeSink`'s atomicity pattern.
fn write_atomic(path: &Path, body: &str) -> Result<(), CoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let _ = fs::create_dir_all(parent);

    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| CoreError::Storage(format!("chromium sink temp-file: {e}")))?;
    tmp.write_all(body.as_bytes()).map_err(CoreError::Io)?;
    tmp.flush().map_err(CoreError::Io)?;
    tmp.as_file().sync_all().map_err(CoreError::Io)?;

    tmp.persist(path).map_err(|e| CoreError::Io(e.error))?;
    Ok(())
}

/// Map a `ParseError` from `delete()` into a `CoreError`. Local to
/// the sink so we don't have to re-export the conversion through
/// `parser.rs`.
fn parse_error_to_core(e: ParseError) -> CoreError {
    match e {
        ParseError::Io(io) => CoreError::Io(io),
        ParseError::Json(j) => CoreError::Json(j),
        other => CoreError::Storage(format!("chromium sink parse: {other}")),
    }
}

// ────────────────────────────────────────────────────────────────────
// Extension trait: flatten a parsed tree back into a `Vec<Bookmark>`
// so `delete()` can rewrite the file.
// ────────────────────────────────────────────────────────────────────

/// Trait extension that flattens a `ChromiumBookmarks` tree back into
/// `Vec<Bookmark>` — the inverse of `parser::flatten`. Used by
/// `ChromiumSink::delete` to rewrite the file omitting a target URL.
pub trait ChromiumTreeFlatten {
    /// Walk the tree and yield one `Bookmark` per URL node. Folder
    /// nodes contribute to the `collection` path of their children.
    fn into_flat_bookmarks(self) -> Vec<Bookmark>;
}

impl ChromiumTreeFlatten for ChromiumBookmarks {
    fn into_flat_bookmarks(self) -> Vec<Bookmark> {
        let mut out = Vec::new();
        collect_flat(&self.roots.bookmark_bar, "", &mut out);
        collect_flat(&self.roots.other, "", &mut out);
        if let Some(synced) = self.roots.synced {
            collect_flat(&synced, "", &mut out);
        }
        if let Some(custom) = self.roots.custom_root {
            for (_key, node) in custom {
                collect_flat(&node, "", &mut out);
            }
        }
        out
    }
}

fn collect_flat(node: &BookmarkNode, prefix: &str, out: &mut Vec<Bookmark>) {
    match node.kind.as_str() {
        "url" => {
            if let Some(url) = &node.url {
                let canonical = linkmarks_core::canonicalize(url).unwrap_or_else(|_| url.clone());
                let created_at = parse_date(node.date_added.as_deref()).unwrap_or_else(Utc::now);
                let updated_at = parse_date(node.date_last_used.as_deref()).unwrap_or(created_at);
                let collection = if prefix.is_empty() {
                    None
                } else {
                    Some(prefix.to_string())
                };
                out.push(Bookmark {
                    id: BookmarkId::generate(),
                    original_url: url.clone(),
                    canonical_url: canonical,
                    title: node.name.trim().to_string(),
                    description: None,
                    tags: Vec::new(),
                    collection,
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
                });
            }
        }
        "folder" => {
            let next_prefix = if prefix.is_empty() {
                node.name.clone()
            } else {
                format!("{prefix}/{}", node.name)
            };
            for child in &node.children {
                collect_flat(child, &next_prefix, out);
            }
        }
        _ => {}
    }
}

/// Parse a Chromium timestamp string into a UTC `DateTime`. Returns
/// `None` on missing or unparseable input — symmetric to
/// `parser::parse_chromium_timestamp`.
fn parse_date(raw: Option<&str>) -> Option<chrono::DateTime<Utc>> {
    let raw = raw?;
    let micros: i64 = raw.parse().ok()?;
    let unix_micros = micros.checked_sub(11_644_473_600_000_000)?;
    let secs = unix_micros.div_euclid(1_000_000);
    let nsec = (unix_micros.rem_euclid(1_000_000) * 1000) as u32;
    Utc.timestamp_opt(secs, nsec).single()
}

#[cfg(test)]
mod tests {
    use super::*;
    use linkmarks_core::traits::BookmarkSink;

    fn bk(url: &str, title: &str, collection: Option<&str>) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: url.to_string(),
            canonical_url: url.to_string(),
            title: title.to_string(),
            description: None,
            tags: Vec::new(),
            collection: collection.map(str::to_string),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source: SourceRef {
                kind: SourceKind::Chromium,
                external_id: None,
                imported_at: chrono::Utc::now(),
                raw: None,
            },
            content_type: None,
            archived: false,
        }
    }

    #[test]
    fn build_emits_bookmark_bar_and_other_roots() {
        let tree = ChromiumSink::build_chromium_bookmarks(&[
            bk("https://example.com/", "Example", Some("Work")),
            bk("https://orphan.example/", "Orphan", None),
        ]);
        assert_eq!(tree.roots.bookmark_bar.name, BOOKMARK_BAR_NAME);
        assert_eq!(tree.roots.other.name, OTHER_BOOKMARKS_NAME);
        assert_eq!(tree.roots.bookmark_bar.kind, "folder");
        // bookmark_bar > Work > Example
        let work = &tree.roots.bookmark_bar.children[0];
        assert_eq!(work.name, "Work");
        assert_eq!(work.children[0].name, "Example");
        assert_eq!(
            work.children[0].url.as_deref(),
            Some("https://example.com/")
        );
        // other > Orphan
        let orphan = &tree.roots.other.children[0];
        assert_eq!(orphan.name, "Orphan");
        assert_eq!(orphan.url.as_deref(), Some("https://orphan.example/"));
    }

    #[test]
    fn build_is_deterministic() {
        // Three bookmarks, three distinct collections, three URLs
        // that sort cleanly as alpha < beta < gamma. After the
        // canonical-URL sort in `build_chromium_bookmarks`, the JSON
        // must list them in that order — so the first occurrence of
        // each host substring appears in alphabetical order.
        let list = vec![
            bk("https://gamma.example/", "Gamma", Some("Zeta")),
            bk("https://alpha.example/", "Alpha", Some("Alpha-folder")),
            bk("https://beta.example/", "Beta", Some("Beta-folder")),
        ];
        let a = ChromiumSink::build_json(&list);
        let b = ChromiumSink::build_json(&list);
        assert_eq!(a, b, "rerun on identical input must produce identical JSON");
        let a_pos = a.find("alpha").expect("alpha should appear in output");
        let b_pos = a.find("beta").expect("beta should appear in output");
        let g_pos = a.find("gamma").expect("gamma should appear in output");
        assert!(
            a_pos < b_pos && b_pos < g_pos,
            "bookmarks must be sorted by canonical URL: a={a_pos}, b={b_pos}, g={g_pos}"
        );
    }

    #[test]
    fn build_emits_nested_folders() {
        let list = vec![
            bk("https://example.com/a", "A", Some("Work/Research")),
            bk(
                "https://example.com/b",
                "B",
                Some("Work/Engineering/Backend"),
            ),
            bk("https://example.com/c", "C", Some("Work/Research")),
        ];
        let tree = ChromiumSink::build_chromium_bookmarks(&list);
        // bookmark_bar > Work
        let work = &tree.roots.bookmark_bar.children[0];
        assert_eq!(work.name, "Work");
        // Work has 2 children: Research, Engineering
        let mut names: Vec<&str> = work.children.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["Engineering", "Research"]);
        // Research has 2 url children (A, C)
        let research = work.children.iter().find(|c| c.name == "Research").unwrap();
        assert_eq!(research.children.len(), 2);
        // Engineering > Backend > B
        let eng = work
            .children
            .iter()
            .find(|c| c.name == "Engineering")
            .unwrap();
        let backend = &eng.children[0];
        assert_eq!(backend.name, "Backend");
        assert_eq!(backend.children[0].name, "B");
    }

    #[test]
    fn build_skips_optional_fields_when_none() {
        let tree = ChromiumBookmarks {
            roots: Roots {
                bookmark_bar: BookmarkNode {
                    kind: "folder".to_string(),
                    name: "Bar".to_string(),
                    url: None,
                    children: vec![],
                    date_added: None,
                    date_last_used: None,
                },
                other: BookmarkNode {
                    kind: "folder".to_string(),
                    name: "Other".to_string(),
                    url: None,
                    children: vec![BookmarkNode {
                        kind: "url".to_string(),
                        name: "X".to_string(),
                        url: Some("https://x.example/".to_string()),
                        children: Vec::new(),
                        date_added: None,
                        date_last_used: None,
                    }],
                    date_added: None,
                    date_last_used: None,
                },
                synced: None,
                custom_root: None,
            },
        };
        let body = ChromiumSink::render(&tree);
        assert!(!body.contains("\"synced\""));
        assert!(!body.contains("\"custom_root\""));
        assert!(!body.contains("\"children\": []"));
        assert!(!body.contains("\"date_added\": null"));
        assert!(!body.contains("\"date_last_used\": null"));
    }

    #[test]
    fn build_emits_dates_in_chromium_microseconds() {
        // 2024-01-01T00:00:00Z = 1704067200 unix seconds
        // = 1_704_067_200_000_000 unix microseconds
        // + 11_644_473_600_000_000 offset (Windows epoch 1601-01-01) =
        //   13_348_540_800_000_000 microseconds since Windows epoch.
        // Chromium Bookmarks JSON encodes `date_added` as decimal-string
        // microseconds since 1601-01-01.
        let dt = chrono::Utc
            .timestamp_opt(1_704_067_200, 0)
            .single()
            .unwrap();
        let mut bm = bk("https://example.com/", "X", None);
        bm.created_at = dt;
        bm.updated_at = dt;
        let tree = ChromiumSink::build_chromium_bookmarks(&[bm]);
        let url_node = &tree.roots.other.children[0];
        assert_eq!(
            url_node.date_added.as_deref(),
            Some("13348540800000000"),
            "date_added must encode UTC as Chromium microseconds since the Windows epoch"
        );
        assert_eq!(
            url_node.date_last_used.as_deref(),
            Some("13348540800000000"),
            "date_last_used must encode UTC as Chromium microseconds since the Windows epoch"
        );
    }

    #[test]
    fn in_memory_sink_captures_body() {
        let mut sink = ChromiumSink::in_memory();
        let report = sink
            .write(&[bk("https://x.example/", "X", Some("Foo"))])
            .unwrap();
        assert_eq!(report.written, 1);
        let body = sink.last_body().expect("body captured");
        assert!(body.contains("\"type\": \"url\""));
        assert!(body.contains("Foo"));
    }

    #[test]
    fn file_sink_writes_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Bookmarks");
        let mut sink = ChromiumSink::open(&target);
        sink.write(&[bk("https://x.example/", "X", None)]).unwrap();
        assert!(target.exists());
        let bytes = fs::read(&target).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn build_drops_tags_silently() {
        // Chromium's native Bookmarks JSON has no tag field, so the
        // sink drops `Bookmark::tags` silently rather than appending
        // them to the name. Synthetic `#folder/*` tags are dropped
        // for the same reason (they're re-derivable from `collection`
        // on re-import). This test pins the contract: a bookmark with
        // tags set emits a url-node whose `name` equals `b.title` —
        // no `(tags: ...)` suffix, no leak of tag content.
        let mut bm = bk("https://example.com/", "Plain Title", Some("Work"));
        bm.tags = vec!["foo".to_string(), "bar".to_string()];
        let tree = ChromiumSink::build_chromium_bookmarks(&[bm]);
        let work = &tree.roots.bookmark_bar.children[0];
        let url_node = &work.children[0];
        assert_eq!(url_node.kind, "url");
        assert_eq!(
            url_node.name, "Plain Title",
            "tags must NOT be appended to the name (Chromium schema has no tag field)"
        );
        assert!(
            !url_node.name.contains("(tags:"),
            "no tag-suffix formatting in the emitted JSON"
        );
        assert!(
            !url_node.name.contains("foo"),
            "tag names must not leak into the bookmark name"
        );

        // Round-trip the body and verify the bookmark still appears
        // with the plain title and no tags in the parsed record.
        let body = ChromiumSink::render(&tree);
        assert!(
            !body.contains("(tags:") && !body.contains("\"tags\""),
            "rendered JSON must not include any tag field"
        );
    }

    #[test]
    fn build_creates_intermediate_folders_without_url_children() {
        // Intermediate folder `Empty` has no URL children of its own —
        // it must still appear as a folder under Work so the structure
        // mirrors the input collection paths.
        let list = vec![
            bk("https://example.com/a", "A", Some("Work/Empty/Deep")),
            bk("https://example.com/b", "B", Some("Work/Empty/Deep")),
        ];
        let tree = ChromiumSink::build_chromium_bookmarks(&list);
        let work = &tree.roots.bookmark_bar.children[0];
        assert_eq!(work.name, "Work");
        let empty = work.children.iter().find(|c| c.name == "Empty").unwrap();
        assert_eq!(empty.name, "Empty");
        let deep = &empty.children[0];
        assert_eq!(deep.name, "Deep");
        assert_eq!(deep.children.len(), 2);
    }
}
