//! Round-trip tests: parse fixture → write to disk → re-parse →
//! assert canonical URLs and titles survive. Tags may shuffle
//! order; folder synthetic tags may be absent if the round-trip
//! emitted different folder groupings. Canonical URLs and titles
//! must match exactly.

use linkmarks_bridge_netscape::parser::{parse, parse_file};
use linkmarks_bridge_netscape::NetscapeSink;
use linkmarks_bridge_netscape::NetscapeSource;
use linkmarks_core::traits::BookmarkSource;

fn fixture(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn canonical_id_set(
    parsed: &linkmarks_bridge_netscape::NetscapeBookmarks,
) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = parsed
        .bookmarks
        .iter()
        .map(|b| (b.canonical_url.clone(), b.title.clone()))
        .collect();
    v.sort();
    v
}

fn round_trip(fixture_name: &str) {
    let original = parse_file(&fixture(fixture_name)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("roundtrip.html");
    let (report, _body) = NetscapeSink::write_to(&target, &original.bookmarks).unwrap();
    assert_eq!(report.written, original.bookmarks.len());
    let re_parsed = parse_file(&target).unwrap();
    let original_set = canonical_id_set(&original);
    let re_set = canonical_id_set(&re_parsed);
    assert_eq!(
        original_set, re_set,
        "round-trip drift for {fixture_name}: original={original_set:?} re={re_set:?}"
    );
}

#[test]
fn round_trip_minimal() {
    round_trip("minimal.html");
}

#[test]
fn round_trip_nested() {
    round_trip("nested.html");
}

#[test]
fn round_trip_with_tags() {
    round_trip("with-tags.html");
}

#[test]
fn round_trip_with_special_chars() {
    round_trip("with-special-chars.html");
}

#[test]
fn round_trip_with_descriptions() {
    round_trip("with-descriptions.html");
}

#[test]
fn round_trip_empty() {
    round_trip("empty.html");
}

#[test]
fn round_trip_pinboard_export() {
    round_trip("pinboard-export.html");
}

#[test]
fn round_trip_preserves_descriptions() {
    let original = parse_file(&fixture("with-descriptions.html")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("rt.html");
    NetscapeSink::write_to(&target, &original.bookmarks).unwrap();
    let re = parse_file(&target).unwrap();
    // Two of the four fixtures have descriptions; both must survive.
    let orig_descs: Vec<_> = original
        .bookmarks
        .iter()
        .filter_map(|b| {
            b.description
                .as_ref()
                .map(|d| (b.canonical_url.clone(), d.clone()))
        })
        .collect();
    let re_descs: Vec<_> = re
        .bookmarks
        .iter()
        .filter_map(|b| {
            b.description
                .as_ref()
                .map(|d| (b.canonical_url.clone(), d.clone()))
        })
        .collect();
    assert_eq!(orig_descs.len(), re_descs.len());
    assert_eq!(orig_descs, re_descs);
}

#[test]
fn round_trip_via_source_impl() {
    let path = fixture("with-tags.html");
    let src = NetscapeSource::open(&path).unwrap();
    let list = src.list().unwrap();
    assert_eq!(list.len(), 4);
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("rt.html");
    NetscapeSink::write_to(&target, &list).unwrap();
    let re_src = NetscapeSource::open(&target).unwrap();
    let re_list = re_src.list().unwrap();
    assert_eq!(list.len(), re_list.len());
    let mut orig: Vec<String> = list.iter().map(|b| b.canonical_url.clone()).collect();
    let mut re_: Vec<String> = re_list.iter().map(|b| b.canonical_url.clone()).collect();
    orig.sort();
    re_.sort();
    assert_eq!(orig, re_);
}

#[test]
fn parse_then_write_is_byte_deterministic() {
    // Two consecutive parse→write cycles on the same input must
    // produce identical HTML bytes.
    let original = parse_file(&fixture("nested.html")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let p1 = dir.path().join("a.html");
    let p2 = dir.path().join("b.html");
    NetscapeSink::write_to(&p1, &original.bookmarks).unwrap();
    let after1 = parse_file(&p1).unwrap();
    eprintln!(
        "AFTER1 collections: {:?}",
        after1
            .bookmarks
            .iter()
            .map(|b| (b.canonical_url.clone(), b.collection.clone()))
            .collect::<Vec<_>>()
    );
    NetscapeSink::write_to(&p2, &after1.bookmarks).unwrap();
    let b1 = std::fs::read(&p1).unwrap();
    let b2 = std::fs::read(&p2).unwrap();
    assert_eq!(b1, b2);
}

#[test]
fn round_trip_with_full_input_string() {
    // Sanity check: a small inline HTML string round-trips
    // without touching any fixture file.
    let html = r#"<!DOCTYPE NETSCAPE-Bookmark-file-1>
<DL><p>
    <DT><A HREF="https://x.example/">X</A>
    <DT><A HREF="https://y.example/">Y</A>
</DL><p>"#;
    let parsed = parse(html.as_bytes()).unwrap();
    assert_eq!(parsed.bookmarks.len(), 2);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("x.html");
    NetscapeSink::write_to(&p, &parsed.bookmarks).unwrap();
    let re = parse_file(&p).unwrap();
    let mut orig: Vec<_> = parsed
        .bookmarks
        .iter()
        .map(|b| b.canonical_url.clone())
        .collect();
    let mut re_: Vec<_> = re
        .bookmarks
        .iter()
        .map(|b| b.canonical_url.clone())
        .collect();
    orig.sort();
    re_.sort();
    assert_eq!(orig, re_);
}
