//! Tests for the Netscape sink. Exercises both the in-memory and
//! file-bound paths. Every fixture lives in-tree under
//! `tests/fixtures/`.

use chrono::{TimeZone, Utc};
use linkmarks_bridge_netscape::NetscapeSink;
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef, Tag};
use linkmarks_core::traits::BookmarkSink;

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
fn empty_input_produces_valid_doctype() {
    let mut sink = NetscapeSink::in_memory();
    let r = sink.write(&[]).unwrap();
    assert_eq!(r.written, 0);
    let body = sink.last_body().unwrap();
    assert!(body.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
    assert!(body.contains("<DL><p>"));
    assert!(body.ends_with("</DL><p>\n"));
}

#[test]
fn single_bookmark_renders_a_tag() {
    let mut sink = NetscapeSink::in_memory();
    sink.write(&[bk("https://example.com/", "Example")]).unwrap();
    let body = sink.last_body().unwrap();
    assert!(body.contains("HREF=\"https://example.com/\""));
    assert!(body.contains(">Example</A>"));
    assert!(body.contains("ADD_DATE=\"1700000000\""));
}

#[test]
fn output_is_byte_deterministic_for_same_input() {
    let list = vec![
        bk("https://z.example/", "Z"),
        bk("https://a.example/", "A"),
        bk("https://m.example/", "M"),
    ];
    let mut s1 = NetscapeSink::in_memory();
    let mut s2 = NetscapeSink::in_memory();
    s1.write(&list).unwrap();
    s2.write(&list).unwrap();
    assert_eq!(s1.last_body().unwrap(), s2.last_body().unwrap());
}

#[test]
fn output_is_sorted_by_canonical_url() {
    let list = vec![
        bk("https://z.example/", "Z"),
        bk("https://a.example/", "A"),
        bk("https://m.example/", "M"),
    ];
    let mut sink = NetscapeSink::in_memory();
    sink.write(&list).unwrap();
    let body = sink.last_body().unwrap();
    let a = body.find("https://a.example").unwrap();
    let m = body.find("https://m.example").unwrap();
    let z = body.find("https://z.example").unwrap();
    assert!(a < m && m < z);
}

#[test]
fn special_chars_in_title_and_url_are_escaped() {
    let mut b = bk("https://example.com/?q=Rust&CLI", "AT&T <hi> \"world\"");
    b.title = "AT&T <hi> \"world\"".to_string();
    b.original_url = "https://example.com/?q=Rust&CLI".to_string();
    let mut sink = NetscapeSink::in_memory();
    sink.write(&[b]).unwrap();
    let body = sink.last_body().unwrap();
    assert!(body.contains("&amp;"));
    assert!(body.contains("&lt;hi&gt;"));
    assert!(body.contains("&quot;world&quot;"));
    // The raw '<' must not appear outside entity refs.
    assert!(!body.contains("AT&T <hi>"));
}

#[test]
fn tags_attribute_omits_folder_synthetic() {
    let mut b = bk("https://example.com/", "X");
    b.tags = vec![
        "rust".to_string(),
        "#folder/work".to_string(),
        "cli".to_string(),
    ];
    b.collection = Some("Work".to_string());
    let mut sink = NetscapeSink::in_memory();
    sink.write(&[b]).unwrap();
    let body = sink.last_body().unwrap();
    let tags_line = body.lines().find(|l| l.contains("TAGS=")).unwrap();
    assert!(tags_line.contains("rust"));
    assert!(tags_line.contains("cli"));
    assert!(!tags_line.contains("#folder"));
}

#[test]
fn folder_collection_emits_h3_open_and_close() {
    let mut b = bk("https://example.com/", "X");
    b.collection = Some("Work/Research".to_string());
    let mut sink = NetscapeSink::in_memory();
    sink.write(&[b]).unwrap();
    let body = sink.last_body().unwrap();
    assert!(body.contains("<H3>Work/Research</H3>"));
    assert!(body.contains("<DL><p>"));
    // Body must contain a closing </DL> for the folder scope.
    assert!(body.matches("</DL><p>").count() >= 2);
}

#[test]
fn write_to_persists_atomically_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("bookmarks.html");
    let list = vec![bk("https://example.com/", "Disk")];
    let mut sink = NetscapeSink::open(&target);
    let report = sink.write(&list).unwrap();
    assert_eq!(report.written, 1);
    assert!(target.exists());
    let bytes = std::fs::read(&target).unwrap();
    let body = String::from_utf8(bytes).unwrap();
    assert!(body.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
    assert!(body.contains("Disk"));
}

#[test]
fn write_to_overwrites_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("bookmarks.html");
    std::fs::write(&target, "stale content").unwrap();
    let mut sink = NetscapeSink::open(&target);
    sink.write(&[bk("https://example.com/", "X")]).unwrap();
    let body = std::fs::read_to_string(&target).unwrap();
    assert!(!body.contains("stale content"));
    assert!(body.starts_with("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
}

#[test]
fn description_renders_as_dd_block() {
    let mut b = bk("https://example.com/", "T");
    b.description = Some("Hello world & friends".to_string());
    let mut sink = NetscapeSink::in_memory();
    sink.write(&[b]).unwrap();
    let body = sink.last_body().unwrap();
    assert!(body.contains("<DD>Hello world &amp; friends</DD>"));
}

#[test]
fn tag_newtype_round_trip_via_sink() {
    // Tag::new normalizes lowercase / trim.
    let tag = Tag::new("  Rust  ").unwrap();
    let mut b = bk("https://example.com/", "T");
    b.tags = vec![tag.0.clone()];
    let mut sink = NetscapeSink::in_memory();
    sink.write(&[b]).unwrap();
    let body = sink.last_body().unwrap();
    assert!(body.contains("TAGS=\"rust\""));
}