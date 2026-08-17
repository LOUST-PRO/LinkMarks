//! Parser for the Netscape bookmark HTML interchange format.
//!
//! ## Format summary
//!
//! ```html
//! <!DOCTYPE NETSCAPE-Bookmark-file-1>
//! <META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
//! <TITLE>Bookmarks</TITLE>
//! <H1>Bookmarks</H1>
//! <DL><p>
//!     <DT><H3 ADD_DATE="123" LAST_MODIFIED="124">Folder</H3>
//!     <DL><p>
//!         <DT><A HREF="https://example.com/" ADD_DATE="123"
//!              LAST_MODIFIED="124" TAGS="a,b,c">Title</A>
//!         <DD>Description</DD>
//!     </DL><p>
//! </DL><p>
//! ```
//!
//! ## Parser strategy
//!
//! We use `quick-xml`'s pull-parser events. The Netscape grammar is
//! predictable enough that we walk the event stream once and
//! maintain:
//!
//! - a folder-path stack (innermost last)
//! - the in-progress bookmark (HREF, title, attributes) which
//!   becomes a `pending` candidate after `</A>`
//! - the in-progress `<DD>` description
//!
//! On `</DD>` we attach the description to the latest pending
//! bookmark and push it onto the accumulator. If `</A>` is
//! followed by another `<DT>` without a `<DD>` in between (i.e., a
//! bookmark with no description), the next `Start(A)` or `Start(H3)`
//! flushes the prior pending bookmark without description.
//!
//! `<p>` tags and the `<!DOCTYPE ...>` declaration are skipped.
//!
//! ## Folder flattening (v1)
//!
//! Nested `<DT><H3>` folders are **flattened** into synthetic
//! `#folder/<name>` tags on each contained bookmark. The folder
//! path is also preserved on `Bookmark::collection` for human
//! display. A proper `Collection` materialization is scheduled
//! for a future release. Round-trip identity is preserved for
//! canonical URLs and titles; folder tags and the `collection`
//! string may shuffle order.
//!
//! ## Errors
//!
//! The parser is tolerant. Missing `HREF`, missing attributes,
//! unknown tags, unparseable dates are reported as
//! `ParseError::Partial` per element. The bulk parse still
//! returns the bookmarks it could recover.

use chrono::{DateTime, TimeZone, Utc};
use linkmarks_core::canonicalize;
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use quick_xml::events::attributes::Attribute;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use std::path::{Path, PathBuf};

use crate::errors::BridgeError;

/// Errors from parsing Netscape HTML.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// I/O failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Quick-XML parse failure.
    #[error("xml: {0}")]
    Xml(#[from] quick_xml::Error),
    /// XML attribute decode failure.
    #[error("attr decode: {0}")]
    Attr(#[from] quick_xml::events::attributes::AttrError),
    /// A per-bookmark failure (non-fatal).
    #[error("partial failure at {element}: {reason}")]
    Partial {
        /// URL / HREF of the offending element.
        element: String,
        /// Human-readable reason.
        reason: String,
    },
}

/// Top-level shape of a parsed Netscape bookmark file.
#[derive(Debug, Default)]
pub struct NetscapeBookmarks {
    /// Flat list of normalized bookmarks.
    pub bookmarks: Vec<Bookmark>,
    /// Non-fatal element errors collected during the walk.
    pub errors: Vec<ParseError>,
}

impl NetscapeBookmarks {
    /// Number of bookmarks successfully parsed.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bookmarks.len()
    }

    /// `true` if no bookmarks were parsed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bookmarks.is_empty()
    }
}

/// Read a file from disk and parse it.
pub fn parse_file(path: &Path) -> Result<NetscapeBookmarks, ParseError> {
    let bytes = std::fs::read(path)?;
    parse(&bytes)
}

/// Parse raw Netscape HTML bytes into normalized bookmarks.
///
/// `bytes` is the full file content. The function is tolerant of
/// malformed input; per-bookmark failures are collected in
/// `NetscapeBookmarks::errors`.
pub fn parse(bytes: &[u8]) -> Result<NetscapeBookmarks, ParseError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().trim_text(true);
    // Netscape bookmark HTML has `<DT>`, `<p>`, and bare `<META>`
    // tags that don't have explicit closes (HTML-style void
    // elements). Disable strict end-name checking so quick-xml
    // doesn't error on the implicit-close style.
    reader.config_mut().check_end_names = false;

    let mut acc = NetscapeBookmarks::default();
    let mut state = ParseState::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::DocType(_)
            | Event::Decl(_)
            | Event::Comment(_)
            | Event::CData(_)
            | Event::PI(_) => {
                // Ignore.
            }
            Event::GeneralRef(ref_) => {
                // quick-xml 0.41+ emits character references as a
                // separate event instead of folding them into the
                // surrounding Text. The content here is the bytes
                // between `&` and `;`, e.g. `amp` for `&amp;`,
                // `#160` for `&#160;`. We feed those bytes through
                // the same decoder the text path uses.
                let entity_cow = ref_.into_inner();
                let entity_bytes: &[u8] = match &entity_cow {
                    std::borrow::Cow::Borrowed(b) => b,
                    std::borrow::Cow::Owned(b) => b.as_slice(),
                };
                let entity = std::str::from_utf8(entity_bytes).unwrap_or("");
                let resolved = resolve_entity(entity).unwrap_or_else(|| {
                    if let Some(rest) = entity.strip_prefix('#') {
                        decode_numeric_entity(rest).unwrap_or_default()
                    } else {
                        String::new()
                    }
                });
                if !resolved.is_empty() {
                    if state.collecting_description {
                        state.description_buf.push_str(&resolved);
                    } else {
                        // Concatenate without separator — see the
                        // rationale in `handle_text`. The
                        // surrounding Text events already include
                        // their own spacing where applicable.
                        state.title_buf.push_str(&resolved);
                    }
                }
            }
            Event::Start(start) => {
                handle_start(&start, &mut state, &mut acc);
            }
            Event::Empty(empty) => {
                // Self-closing tags are extremely rare in Netscape
                // format but we tolerate them. Apply the same
                // handler as Start, then close immediately.
                handle_start(&empty, &mut state, &mut acc);
                handle_end(empty.name().as_ref(), &mut state, &mut acc);
            }
            Event::Text(text) => {
                handle_text(&text, &mut state)?;
            }
            Event::End(end) => {
                handle_end(end.name().as_ref(), &mut state, &mut acc);
            }
        }
        buf.clear();
    }

    // Flush any trailing pending bookmark that never had a
    // description attached.
    state.flush_pending(&mut acc);

    Ok(acc)
}

/// Helper: parse + flatten in one call.
pub fn parse_and_flatten(path: &Path) -> Result<NetscapeBookmarks, ParseError> {
    parse_file(path)
}

fn handle_start(start: &BytesStart<'_>, state: &mut ParseState, acc: &mut NetscapeBookmarks) {
    let name = lower_owned(start.name().as_ref());
    match name.as_str() {
        // `<DT>` and `<p>` are HTML-style void/marker elements in
        // Netscape format. We treat them as no-ops and ignore the
        // corresponding (missing) End event.
        "dt" | "p" | "meta" | "title" | "h1" => {
            // No state change.
        }
        "dl" => {
            state.dl_depth += 1;
            if let Some(name) = state.pending_folder_name.take() {
                // Latch the FIRST H3 at the outer DL (depth 1) as the
                // implicit root collection name. Subsequent sibling
                // folders get prepended with it (Pinboard / Firefox
                // "Bookmarks Menu" convention).
                if state.dl_depth == 2 && state.root_collection_name.is_none() {
                    state.root_collection_name = Some(name.clone());
                }
                state.folder_stack.push(name);
            }
        }
        "a" => {
            // If a prior DT-anchor is still pending without a
            // description, flush it as-is.
            if state.pending_bookmark.is_some() {
                state.flush_pending(acc);
            }
            on_open_a(start, state, acc);
        }
        "dd" => {
            // The DD tag opens a description block. The text
            // events between Start(DD) and End(DD) belong to the
            // most-recently-closed `<A>` bookmark.
            state.collecting_description = true;
            state.description_buf.clear();
        }
        "h3" => {
            if state.pending_bookmark.is_some() {
                state.flush_pending(acc);
            }
            state.collecting_description = false;
            state.title_buf.clear();
            state.h3_seen = true;
        }
        _ => {}
    }
}

fn handle_text(text: &BytesText<'_>, state: &mut ParseState) -> Result<(), ParseError> {
    // quick-xml's `unescape()` only knows XML built-ins (&amp; &lt;
    // &gt; &quot; &apos; + numeric refs). It returns `EscapeError` on
    // HTML named entities like `&aacute;`. We bypass it and run our
    // own decoder that handles XML + common HTML5 named entities +
    // numeric refs in one pass.
    //
    // We do NOT insert a space separator between consecutive text
    // chunks (Text + GeneralRef + Text for the same anchor). The
    // content of a `<A>` title is a contiguous run; concatenating
    // without padding preserves fidelity (e.g. `AT&amp;T` → `AT&T`).
    // Earlier versions of this code added a `' '` separator which
    // was wrong: with quick-xml 0.41 the GeneralRef event splits a
    // single Text event that *had* no separator, and our decoder
    // emits the resolved character in-place.
    let raw_bytes: &[u8] = text;
    let raw = std::str::from_utf8(raw_bytes).map_err(|e| ParseError::Partial {
        element: String::new(),
        reason: format!("text not utf-8: {e}"),
    })?;
    let txt = decode_entities(raw);
    if txt.is_empty() {
        return Ok(());
    }
    if state.collecting_description {
        state.description_buf.push_str(&txt);
    } else {
        state.title_buf.push_str(&txt);
    }
    Ok(())
}

/// Decode XML + HTML5 named entities + numeric character references.
///
/// `quick_xml::BytesText::unescape()` only handles the XML built-ins
/// (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`). Netscape bookmark
/// files commonly contain HTML5 named entities (`&aacute;`, `&copy;`,
/// `&euro;`, `&mdash;`, …). We map the ones that appear in real
/// exports; unknown named entities are passed through verbatim.
fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = input[i..].chars().next().unwrap();
        if c != '&' {
            out.push(c);
            i += c.len_utf8();
            continue;
        }
        // Try to read an entity reference up to a `;` or non-name char.
        let start = i + 1;
        let mut end = start;
        let mut found_semi = false;
        while end < bytes.len() && end - start <= 12 {
            let b = bytes[end];
            if b == b';' {
                found_semi = true;
                break;
            }
            if !(b.is_ascii_alphanumeric() || b == b'#') {
                break;
            }
            end += 1;
        }
        if found_semi {
            let entity = &input[start..end];
            if let Some(decoded) = resolve_entity(entity) {
                out.push_str(&decoded);
                i = end + 1;
                continue;
            }
        }
        // Unknown or malformed: emit literal.
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn resolve_entity(entity: &str) -> Option<String> {
    let resolved: String = match entity {
        // XML built-ins.
        "amp" => "&".into(),
        "lt" => "<".into(),
        "gt" => ">".into(),
        "quot" => "\"".into(),
        "apos" => "'".into(),
        // Whitespace / punctuation.
        "nbsp" => "\u{00A0}".into(),
        "copy" => "©".into(),
        "reg" => "®".into(),
        "trade" => "™".into(),
        "mdash" => "—".into(),
        "ndash" => "–".into(),
        "hellip" => "…".into(),
        "laquo" => "«".into(),
        "raquo" => "»".into(),
        "lsquo" => "\u{2018}".into(),
        "rsquo" => "\u{2019}".into(),
        "ldquo" => "\u{201C}".into(),
        "rdquo" => "\u{201D}".into(),
        "middot" => "·".into(),
        "bull" => "•".into(),
        // Currency.
        "euro" => "€".into(),
        "pound" => "£".into(),
        "yen" => "¥".into(),
        "cent" => "¢".into(),
        // Common accented latin (lowercase + uppercase).
        "aacute" => "á".into(),
        "eacute" => "é".into(),
        "iacute" => "í".into(),
        "oacute" => "ó".into(),
        "uacute" => "ú".into(),
        "agrave" => "à".into(),
        "egrave" => "è".into(),
        "igrave" => "ì".into(),
        "ograve" => "ò".into(),
        "ugrave" => "ù".into(),
        "acirc" => "â".into(),
        "ecirc" => "ê".into(),
        "icirc" => "î".into(),
        "ocirc" => "ô".into(),
        "ucirc" => "û".into(),
        "auml" => "ä".into(),
        "euml" => "ë".into(),
        "iuml" => "ï".into(),
        "ouml" => "ö".into(),
        "uuml" => "ü".into(),
        "ntilde" => "ñ".into(),
        "ccedil" => "ç".into(),
        _ => {
            // Numeric: #NN or #xNN.
            if let Some(stripped) = entity.strip_prefix('#') {
                let cp = if let Some(hex) = stripped
                    .strip_prefix('x')
                    .or_else(|| stripped.strip_prefix('X'))
                {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    stripped.parse::<u32>().ok()
                };
                if let Some(cp) = cp {
                    if let Some(c) = char::from_u32(cp) {
                        return Some(c.to_string());
                    }
                }
            }
            return None;
        }
    };
    Some(resolved)
}

/// Decode a numeric character reference body — i.e. the digits after
/// `&#`, like `160` (decimal) or `xA0` (hex). Returns the single char.
fn decode_numeric_entity(body: &str) -> Option<String> {
    let body = body.trim();
    if let Some(hex) = body.strip_prefix('x').or_else(|| body.strip_prefix('X')) {
        if let Ok(n) = u32::from_str_radix(hex, 16) {
            return char::from_u32(n).map(|c| c.to_string());
        }
    }
    if let Ok(n) = body.parse::<u32>() {
        return char::from_u32(n).map(|c| c.to_string());
    }
    None
}

fn handle_end(name_bytes: &[u8], state: &mut ParseState, acc: &mut NetscapeBookmarks) {
    let name = lower_owned(name_bytes);
    match name.as_str() {
        "a" => {
            on_close_a(state);
        }
        "h3" => {
            on_close_h3(state);
        }
        "dd" => {
            // Attach description to the pending bookmark and
            // push to the accumulator.
            state.flush_with_description(acc);
            state.collecting_description = false;
        }
        "dl" => {
            if state.dl_depth > 0 {
                state.dl_depth -= 1;
            }
            // Invariant: folder_stack.len() == dl_depth - 1 after
            // the pop. Trim down to that length.
            let desired = state.dl_depth.saturating_sub(1) as usize;
            while state.folder_stack.len() > desired {
                state.folder_stack.pop();
            }
            // Flush any pending bookmark whose parent scope is closing.
            if state.pending_bookmark.is_some() {
                state.flush_pending(acc);
            }
        }
        // No-op void/marker tags.
        "dt" | "p" | "meta" | "title" | "h1" => {}
        _ => {}
    }
}

fn on_open_a(start: &BytesStart<'_>, state: &mut ParseState, acc: &mut NetscapeBookmarks) {
    let mut href: Option<String> = None;
    let mut add_date: Option<i64> = None;
    let mut last_modified: Option<i64> = None;
    let mut tags: Option<String> = None;

    for attr in start.attributes().with_checks(false) {
        match attr {
            Ok(a) => {
                let key = lower_owned(a.key.as_ref());
                match key.as_str() {
                    "href" => {
                        if let Ok(v) = decode_attr(&a) {
                            href = Some(v);
                        }
                    }
                    "add_date" => {
                        if let Ok(v) = decode_attr(&a) {
                            add_date = v.trim().parse().ok();
                        }
                    }
                    "last_modified" => {
                        if let Ok(v) = decode_attr(&a) {
                            last_modified = v.trim().parse().ok();
                        }
                    }
                    "tags" => {
                        if let Ok(v) = decode_attr(&a) {
                            tags = Some(v);
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => {
                acc.errors.push(ParseError::Attr(e));
            }
        }
    }

    state.pending_href = href;
    state.pending_add_date = add_date;
    state.pending_last_modified = last_modified;
    state.pending_tags = tags;
    state.title_buf.clear();
    state.description_buf.clear();
    state.collecting_description = false;
}

fn on_close_a(state: &mut ParseState) {
    let Some(url) = state.pending_href.take() else {
        // No HREF — typed-internal link. Skip cleanly.
        state.title_buf.clear();
        return;
    };
    if url.is_empty() {
        state.title_buf.clear();
        return;
    }

    let canonical = match canonicalize(&url) {
        Ok(c) => c,
        Err(e) => {
            state.errors.push(ParseError::Partial {
                element: url.clone(),
                reason: format!("canonicalize: {e}"),
            });
            state.title_buf.clear();
            return;
        }
    };

    let add_secs = state.pending_add_date.take().unwrap_or(0);
    let mod_secs = state.pending_last_modified.take().unwrap_or(0);
    let created_at = secs_to_utc(add_secs);
    let mut updated_at = secs_to_utc(mod_secs);
    if updated_at.timestamp() == 0 {
        updated_at = created_at;
    }

    let mut tags_vec: Vec<String> = Vec::new();
    if let Some(s) = state.pending_tags.take() {
        for t in s.split(',') {
            let trimmed = t.trim();
            if !trimmed.is_empty() {
                tags_vec.push(trimmed.to_ascii_lowercase());
            }
        }
    }
    for f in &state.folder_stack {
        tags_vec.push(format!("#folder/{}", f.to_ascii_lowercase()));
    }
    tags_vec.sort();
    tags_vec.dedup();

    let title = state.title_buf.trim().to_string();
    state.title_buf.clear();

    // Pinboard / Firefox "Bookmarks Menu" semantics: when the
    // first H3 at the outer DL names a root collection, every
    // bookmark that lives DIRECTLY under root (no folder_stack
    // entry) gets the root prepended as the implicit parent.
    // Bookmarks nested inside root's subtree keep their natural
    // path. This avoids duplicating the root in deeper paths like
    // "Root/Level1A" re-appearing as "Root/Root/Level1A".
    let collection_str = state
        .folder_stack
        .first()
        .map(|_| state.folder_stack.join("/"))
        .or(state.root_collection_name.clone());
    if state.folder_stack.is_empty() {
        if let Some(root) = &state.root_collection_name {
            tags_vec.push(format!("#folder/{}", root.to_ascii_lowercase()));
        }
    }
    tags_vec.sort();
    tags_vec.dedup();

    let bookmark = Bookmark {
        id: BookmarkId::generate(),
        original_url: url.clone(),
        canonical_url: canonical,
        title,
        description: None,
        tags: tags_vec,
        collection: collection_str,
        created_at,
        updated_at,
        source: SourceRef {
            kind: SourceKind::Netscape,
            external_id: Some(url),
            imported_at: Utc::now(),
            raw: None,
        },
        content_type: None,
        archived: false,
    };

    state.pending_bookmark = Some(bookmark);
}

fn on_close_h3(state: &mut ParseState) {
    let name = state.title_buf.trim().to_string();
    state.title_buf.clear();
    state.collecting_description = false;
    state.h3_seen = false;
    // The folder name attaches to the next `<DL>` opening.
    // We store it as a pending value; `handle_start(DL)` pops it.
    state.pending_folder_name = if name.is_empty() { None } else { Some(name) };
}

fn decode_attr(attr: &Attribute<'_>) -> Result<String, ParseError> {
    // `unescape_value()` is available when the `encoding` feature
    // of quick-xml is OFF (our case — we only enable `serialize`).
    // It decodes UTF-8 then unescapes HTML entities. This is the
    // exact semantic we want: a raw byte-to-string conversion that
    // resolves XML predefined entities. We deliberately do NOT use
    // `normalized_value()` because that additionally applies XML
    // attribute-value normalization (whitespace collapsing), which
    // would corrupt URLs that legitimately contain multiple spaces
    // or other whitespace.
    //
    // Marked deprecated in quick-xml 0.41 in favour of
    // `normalized_value()`, but `decode_and_unescape_value` would
    // require us to manage a `Decoder`, and the warning fires only
    // when the `encoding` feature is OFF (our case). Pin to
    // `unescape_value` for behavioural parity with the v0.36 path;
    // revisit if we ever enable `encoding`.
    #[allow(deprecated)]
    let v = attr.unescape_value()?;
    Ok(v.into_owned())
}

/// Convert a Unix-seconds integer into a UTC `DateTime<Utc>`.
/// `0` and negative values fall back to the Unix epoch.
fn secs_to_utc(secs: i64) -> DateTime<Utc> {
    if secs <= 0 {
        return Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now);
    }
    Utc.timestamp_opt(secs, 0)
        .single()
        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap_or_else(Utc::now))
}

fn lower_owned(b: &[u8]) -> String {
    String::from_utf8_lossy(b).to_ascii_lowercase()
}

/// Internal parser state.
struct ParseState {
    folder_stack: Vec<String>,
    dl_depth: i64,
    h3_seen: bool,
    /// First H3 encountered at the outer DL (depth 1). Acts as the
    /// implicit root collection name; prepended to bookmarks whose
    /// `folder_stack` does not start with it.
    root_collection_name: Option<String>,
    pending_folder_name: Option<String>,
    pending_href: Option<String>,
    pending_add_date: Option<i64>,
    pending_last_modified: Option<i64>,
    pending_tags: Option<String>,
    title_buf: String,
    description_buf: String,
    collecting_description: bool,
    pending_bookmark: Option<Bookmark>,
    errors: Vec<ParseError>,
}

impl ParseState {
    fn new() -> Self {
        Self {
            folder_stack: Vec::new(),
            dl_depth: 0,
            h3_seen: false,
            root_collection_name: None,
            pending_folder_name: None,
            pending_href: None,
            pending_add_date: None,
            pending_last_modified: None,
            pending_tags: None,
            title_buf: String::new(),
            description_buf: String::new(),
            collecting_description: false,
            pending_bookmark: None,
            errors: Vec::new(),
        }
    }

    /// Push the pending bookmark as-is (no description) onto the
    /// accumulator, if any.
    fn flush_pending(&mut self, acc: &mut NetscapeBookmarks) {
        if let Some(b) = self.pending_bookmark.take() {
            acc.bookmarks.push(b);
        }
        self.description_buf.clear();
    }

    /// Push the pending bookmark with the accumulated `<DD>`
    /// description attached.
    fn flush_with_description(&mut self, acc: &mut NetscapeBookmarks) {
        let Some(mut b) = self.pending_bookmark.take() else {
            self.description_buf.clear();
            return;
        };
        let desc = self.description_buf.trim();
        if !desc.is_empty() {
            b.description = Some(desc.to_string());
        }
        self.description_buf.clear();
        acc.bookmarks.push(b);
    }
}

/// Returns the typical default locations for Netscape bookmark
/// HTML files on Linux (v1). Returned paths may or may not exist.
#[must_use]
pub fn discover_default_paths() -> Vec<(SourceKind, PathBuf)> {
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => return Vec::new(),
    };
    let candidates: &[&str] = &[
        "bookmarks.html",
        "Bookmarks.html",
        "Downloads/bookmarks.html",
        "Documents/bookmarks.html",
    ];
    candidates
        .iter()
        .map(|c| home.join(c))
        .filter(|p| p.exists())
        .map(|p| (SourceKind::Netscape, p))
        .collect()
}

/// Map a `ParseError` into `CoreError`.
#[must_use]
pub fn parse_error_to_core(err: ParseError) -> CoreError {
    match err {
        ParseError::Io(io) => CoreError::Io(io),
        ParseError::Xml(s) => CoreError::Storage(format!("netscape xml: {s}")),
        ParseError::Attr(a) => CoreError::Storage(format!("netscape attr: {a}")),
        ParseError::Partial { element, reason } => CoreError::Partial { element, reason },
    }
}

/// Map a `BridgeError` into `CoreError`.
#[must_use]
pub fn bridge_error_to_core(err: BridgeError) -> CoreError {
    err.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_single_a() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><A HREF="https://example.com/" ADD_DATE="100">Example</A>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        assert_eq!(parsed.bookmarks.len(), 1);
        assert_eq!(parsed.bookmarks[0].title, "Example");
        assert_eq!(parsed.bookmarks[0].original_url, "https://example.com/");
    }

    #[test]
    fn parses_folder_with_children() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3 ADD_DATE="100">Work</H3>
    <DL><p>
        <DT><A HREF="https://example.com/a">A</A>
        <DT><A HREF="https://example.com/b">B</A>
    </DL><p>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        assert_eq!(parsed.bookmarks.len(), 2);
        for b in &parsed.bookmarks {
            assert_eq!(b.collection.as_deref(), Some("Work"));
            assert!(
                b.tags.iter().any(|t| t == "#folder/work"),
                "tags were: {:?}",
                b.tags
            );
        }
    }

    #[test]
    fn parses_nested_folders() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><H3>Outer</H3>
    <DL><p>
        <DT><H3>Inner</H3>
        <DL><p>
            <DT><A HREF="https://example.com/x">X</A>
        </DL><p>
    </DL><p>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        assert_eq!(parsed.bookmarks.len(), 1);
        assert_eq!(
            parsed.bookmarks[0].collection.as_deref(),
            Some("Outer/Inner")
        );
        assert!(parsed.bookmarks[0]
            .tags
            .iter()
            .any(|t| t == "#folder/outer"));
        assert!(parsed.bookmarks[0]
            .tags
            .iter()
            .any(|t| t == "#folder/inner"));
    }

    #[test]
    fn parses_tags_attribute() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com/" TAGS="Rust, CLI, Bookmarks">T</A>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        let b = &parsed.bookmarks[0];
        assert!(b.tags.iter().any(|t| t == "rust"));
        assert!(b.tags.iter().any(|t| t == "cli"));
        assert!(b.tags.iter().any(|t| t == "bookmarks"));
    }

    #[test]
    fn parses_with_description() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com/">Title</A>
    <DD>A description of the link.</DD>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        assert_eq!(
            parsed.bookmarks[0].description.as_deref(),
            Some("A description of the link.")
        );
    }

    #[test]
    fn decodes_html_entities() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://example.com/?q=Rust&amp;CLI">AT&amp;T</A>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        let b = &parsed.bookmarks[0];
        assert_eq!(b.title, "AT&T");
        assert!(b.original_url.contains("&"));
    }

    #[test]
    fn records_partial_for_missing_href() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A>NoHref</A>
</DL><p>
"#;
        let parsed = parse(html.as_bytes()).unwrap();
        assert!(parsed.bookmarks.is_empty());
    }

    #[test]
    fn handles_empty_dl() {
        let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL></DL>"#;
        let parsed = parse(html.as_bytes()).unwrap();
        assert!(parsed.bookmarks.is_empty());
        assert!(parsed.errors.is_empty());
    }

    #[test]
    fn sec_to_utc_handles_zero() {
        let t = secs_to_utc(0);
        assert_eq!(t.timestamp(), 0);
    }
}
