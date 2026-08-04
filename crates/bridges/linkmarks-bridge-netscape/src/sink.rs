//! Netscape bookmark HTML writer.
//!
//! ## File format
//!
//! The first line **must** be `<!DOCTYPE NETSCAPE-Bookmark-file-1>`
//! (literally — Pinboard, Linkwarden and Shaarli refuse to import
//! files missing the DOCTYPE). After that:
//!
//! ```html
//! <META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
//! <TITLE>Bookmarks</TITLE>
//! <H1>Bookmarks</H1>
//! <DL><p>
//!     <DT><A HREF="..." ADD_DATE="..." LAST_MODIFIED="..." TAGS="...">Title</A>
//!     <DD>Description</DD>
//! </DL><p>
//! ```
//!
//! ## Determinism
//!
//! Bookmarks are written in a stable order: primary key is
//! `canonical_url` (lexicographic), secondary key is the bookmark
//! `id`. Folder grouping is derived from the `collection` field
//! (`/`-separated). For v1 (folder flattening) we re-emit
//! `Bookmark::collection` as `<DT><H3>` folders when present, and
//! we round-trip the synthetic `#folder/*` tags as fold-back into
//! the folder path of the `collection` field on read.
//!
//! ## Atomicity
//!
//! `NetscapeSink::write_to` builds the full body in memory, writes
//! it to a `tempfile::NamedTempFile` in the same directory, then
//! `persist`s (rename) into place. This guarantees the target
//! file is either fully new or untouched — partial writes never
//! appear.
//!
//! ## Escaping
//!
//! We use `quick_xml::escape::escape` on attribute values and
//! text nodes. URLs that already contain `&` get rewritten to
//! `&amp;`. Quotes in titles become `&quot;`. This matches the
//! way Chrome / Firefox escape their exports.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::Bookmark;
use linkmarks_core::traits::{BookmarkSink, WriteReport};

/// A `BookmarkSink` that emits Netscape bookmark HTML.
pub struct NetscapeSink {
    path: Option<PathBuf>,
    /// When `path` is `None`, the sink is in-memory: `write()`
    /// populates `last_body` instead of touching the disk.
    last_body: Option<String>,
    /// Number of bookmarks most-recently written.
    last_report: Option<WriteReport>,
}

impl NetscapeSink {
    /// Create a sink bound to a target file path. The file need
    /// not exist yet; `write` creates or overwrites.
    #[must_use]
    pub fn open(path: &Path) -> Self {
        Self {
            path: Some(path.to_path_buf()),
            last_body: None,
            last_report: None,
        }
    }

    /// Create an in-memory sink that captures writes into a
    /// `String` rather than touching disk. Useful for tests and
    /// for CLI pipelines that want to chain sinks.
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            path: None,
            last_body: None,
            last_report: None,
        }
    }

    /// Build the HTML body from a slice of bookmarks. Pure
    /// function — does no I/O.
    #[must_use]
    pub fn build_html(bookmarks: &[Bookmark]) -> String {
        let mut body = String::new();
        body.push_str("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
        body.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n");
        body.push_str("<TITLE>Bookmarks</TITLE>\n");
        body.push_str("<H1>Bookmarks</H1>\n");
        body.push_str("<DL><p>\n");

        if bookmarks.is_empty() {
            body.push_str("</DL><p>\n");
            return body;
        }

        // Sort by (canonical_url, id) for deterministic output.
        let mut sorted: Vec<&Bookmark> = bookmarks.iter().collect();
        sorted.sort_by(|a, b| {
            a.canonical_url
                .cmp(&b.canonical_url)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });

        // Group by collection path so consecutive entries share a folder context.
        let mut current_path: Option<String> = None;
        for b in &sorted {
            let folder_path = b.collection.clone();
            // If the folder changed, emit <DT><H3>...</H3><DL>... or close </DL> as needed.
            if folder_path != current_path {
                // Close any previously open DL.
                if current_path.is_some() {
                    body.push_str("    </DL><p>\n");
                }
                current_path = folder_path.clone();
                if let Some(p) = &folder_path {
                    body.push_str(&format_folder_open(p));
                }
            }
            body.push_str(&format_bookmark(b));
        }
        if current_path.is_some() {
            body.push_str("    </DL><p>\n");
        }
        body.push_str("</DL><p>\n");
        body
    }

    /// Write a slice of bookmarks atomically to a path. Returns
    /// the report plus the rendered body (for callers that want to
    /// inspect the output without re-reading the file).
    pub fn write_to(path: &Path, bookmarks: &[Bookmark]) -> Result<(WriteReport, String), CoreError> {
        let body = Self::build_html(bookmarks);
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

impl BookmarkSink for NetscapeSink {
    fn kind(&self) -> linkmarks_core::model::SourceKind {
        linkmarks_core::model::SourceKind::Netscape
    }

    fn write(&mut self, bookmarks: &[Bookmark]) -> Result<WriteReport, CoreError> {
        let body = Self::build_html(bookmarks);
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
        // For Netscape HTML, "delete" means: rewrite the file
        // omitting the bookmark with this external_id (the URL).
        let Some(path) = &self.path else {
            return Err(CoreError::Storage(
                "NetscapeSink::delete requires a file-bound sink (got in-memory)".to_string(),
            ));
        };
        let bytes = fs::read(path).map_err(CoreError::Io)?;
        let parsed = crate::parser::parse(&bytes).map_err(crate::parser::parse_error_to_core)?;
        let remaining: Vec<Bookmark> = parsed
            .bookmarks
            .into_iter()
            .filter(|b| b.original_url.as_str() != external_id)
            .collect();
        Self::write_to(path, &remaining)?;
        Ok(())
    }
}

/// Write `body` atomically to `path` (temp-file + rename in the
/// same directory).
fn write_atomic(path: &Path, body: &str) -> Result<(), CoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let _ = fs::create_dir_all(parent);

    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        CoreError::Storage(format!("netscape sink temp-file: {e}"))
    })?;
    tmp.write_all(body.as_bytes()).map_err(CoreError::Io)?;
    tmp.flush().map_err(CoreError::Io)?;
    tmp.as_file().sync_all().map_err(CoreError::Io)?;

    tmp.persist(path).map_err(|e| CoreError::Io(e.error))?;
    Ok(())
}

/// Emit the folder-open prolog for a `/`-separated path.
fn format_folder_open(path: &str) -> String {
    let mut s = String::new();
    s.push_str("    <DT><H3>");
    s.push_str(&escape_text(path));
    s.push_str("</H3>\n");
    s.push_str("    <DL><p>\n");
    s
}

/// Format one `<DT><A>...</A>` (and optional `<DD>...</DD>`) line.
fn format_bookmark(b: &Bookmark) -> String {
    let mut s = String::new();
    s.push_str("        <DT><A HREF=\"");
    s.push_str(&escape_attr(&b.original_url));
    s.push('"');

    let add = b.created_at.timestamp().max(0);
    let modified = b.updated_at.timestamp().max(0);
    if add > 0 {
        s.push_str(&format!(" ADD_DATE=\"{add}\""));
    }
    if modified > 0 {
        s.push_str(&format!(" LAST_MODIFIED=\"{modified}\""));
    }

    let mut tags: Vec<String> = b
        .tags
        .iter()
        .filter(|t| !t.starts_with("#folder/"))
        .cloned()
        .collect();
    tags.sort();
    tags.dedup();
    if !tags.is_empty() {
        s.push_str(" TAGS=\"");
        s.push_str(&escape_attr(&tags.join(",")));
        s.push('"');
    }

    s.push('>');
    s.push_str(&escape_text(&b.title));
    s.push_str("</A>\n");

    if let Some(desc) = &b.description {
        if !desc.is_empty() {
            s.push_str("        <DD>");
            s.push_str(&escape_text(desc));
            s.push_str("</DD>\n");
        }
    }
    s
}

/// Escape an HTML attribute value (`"`, `&`, `<`, `>`, `'` all
/// become named entities via quick-xml's `escape`).
fn escape_attr(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}

/// Escape HTML text content (same set as attributes).
fn escape_text(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}

/// Convert a bookmark timestamp into Unix seconds (clamped at 0).
#[allow(dead_code)]
fn utc_to_secs(t: DateTime<Utc>) -> i64 {
    t.timestamp().max(0)
}

/// Suppress unused warning when callers don't use utc_to_secs.
#[allow(dead_code)]
fn _suppress_unused(_t: DateTime<Utc>) -> i64 {
    let unix_epoch = Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now);
    unix_epoch.timestamp().max(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use linkmarks_core::model::{BookmarkId, SourceKind, SourceRef};

    fn bk(url: &str, title: &str) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: url.to_string(),
            canonical_url: url.to_string(),
            title: title.to_string(),
            description: None,
            tags: Vec::new(),
            collection: None,
            created_at: Utc.timestamp_opt(1_700_000_000, 0).single().unwrap(),
            updated_at: Utc.timestamp_opt(1_700_000_000, 0).single().unwrap(),
            source: SourceRef {
                kind: SourceKind::Netscape,
                external_id: Some(url.to_string()),
                imported_at: Utc::now(),
                raw: None,
            },
            content_type: None,
            archived: false,
        }
    }

    #[test]
    fn build_html_starts_with_doctype() {
        let body = NetscapeSink::build_html(&[]);
        assert!(body.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
        assert!(body.contains("<DL><p>"));
        assert!(body.ends_with("</DL><p>\n"));
    }

    #[test]
    fn build_html_emits_href_and_title() {
        let list = vec![bk("https://example.com/", "Example")];
        let body = NetscapeSink::build_html(&list);
        assert!(body.contains("HREF=\"https://example.com/\""));
        assert!(body.contains(">Example</A>"));
        assert!(body.contains("ADD_DATE=\"1700000000\""));
    }

    #[test]
    fn build_html_is_deterministic() {
        let list = vec![
            bk("https://zzz.example/", "Z"),
            bk("https://aaa.example/", "A"),
            bk("https://mmm.example/", "M"),
        ];
        let a = NetscapeSink::build_html(&list);
        let b = NetscapeSink::build_html(&list);
        assert_eq!(a, b);
        // Sorted by canonical URL.
        let a_pos = a.find("aaa").unwrap();
        let m_pos = a.find("mmm").unwrap();
        let z_pos = a.find("zzz").unwrap();
        assert!(a_pos < m_pos && m_pos < z_pos);
    }

    #[test]
    fn build_html_escapes_special_chars() {
        let mut b = bk("https://example.com/?a=1&b=2", "AT&T < \"hello\" > 'x'");
        b.title = "AT&T <hi>".to_string();
        b.original_url = "https://example.com/?a=1&b=2".to_string();
        let body = NetscapeSink::build_html(&[b]);
        assert!(body.contains("&amp;"));
        assert!(body.contains("&lt;hi&gt;"));
    }

    #[test]
    fn build_html_with_folder() {
        let mut b = bk("https://example.com/", "Example");
        b.collection = Some("Work/Research".to_string());
        let body = NetscapeSink::build_html(&[b]);
        assert!(body.contains("<H3>Work/Research</H3>"));
    }

    #[test]
    fn build_html_skips_folder_tags_in_t_attr() {
        let mut b = bk("https://example.com/", "Example");
        b.tags = vec!["rust".to_string(), "#folder/work".to_string()];
        b.collection = Some("Work".to_string());
        let body = NetscapeSink::build_html(&[b]);
        // The synthetic folder tag should NOT be re-emitted as a TAGS attr.
        let tags_attr_line = body
            .lines()
            .find(|l| l.contains("TAGS="))
            .expect("TAGS= line");
        assert!(tags_attr_line.contains("rust"));
        assert!(!tags_attr_line.contains("#folder"));
    }

    #[test]
    fn build_html_emits_description_as_dd() {
        let mut b = bk("https://example.com/", "Title");
        b.description = Some("Hello world".to_string());
        let body = NetscapeSink::build_html(&[b]);
        assert!(body.contains("<DD>Hello world</DD>"));
    }
}
