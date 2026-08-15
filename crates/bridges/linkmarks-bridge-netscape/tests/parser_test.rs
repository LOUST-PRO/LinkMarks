//! Tests for the Netscape parser, exercised against on-disk
//! fixtures. All fixtures live in `tests/fixtures/` and the
//! parser is hermetic — no network, no `~/.config` reads.

use linkmarks_bridge_netscape::parser::parse;
use linkmarks_bridge_netscape::NetscapeSource;
use linkmarks_core::traits::BookmarkSource;

fn fixture(name: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests");
    p.push("fixtures");
    p.push(name);
    p
}

fn read_fixture(name: &str) -> Vec<u8> {
    std::fs::read(fixture(name)).expect("read fixture")
}

#[test]
fn parses_minimal_three_bookmarks() {
    let bytes = read_fixture("minimal.html");
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.bookmarks.len(), 3);
    assert!(parsed.errors.is_empty(), "errors: {:?}", parsed.errors);
    let urls: Vec<&str> = parsed
        .bookmarks
        .iter()
        .map(|b| b.original_url.as_str())
        .collect();
    assert!(urls.contains(&"https://example.com/"));
    assert!(urls.contains(&"https://example.org/"));
    assert!(urls.contains(&"https://example.net/"));
}

#[test]
fn parses_minimal_folder_paths() {
    let bytes = read_fixture("minimal.html");
    let parsed = parse(&bytes).unwrap();
    let news: Vec<&str> = parsed
        .bookmarks
        .iter()
        .filter(|b| {
            b.original_url.contains("example.com") || b.original_url.contains("example.org")
        })
        .map(|b| b.collection.as_deref().unwrap_or(""))
        .collect();
    assert!(news.iter().all(|c| *c == "News"));
}

#[test]
fn parses_nested_three_levels() {
    let bytes = read_fixture("nested.html");
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.bookmarks.len(), 10);
    let g = parsed
        .bookmarks
        .iter()
        .find(|b| b.original_url == "https://g.example.com/")
        .unwrap();
    assert_eq!(
        g.collection.as_deref(),
        Some("Root/Level1D/Level2C/Level3A")
    );
    // 4 synthetic folder tags + nothing else.
    assert_eq!(g.tags.len(), 4);
    assert!(g.tags.contains(&"#folder/root".to_string()));
    assert!(g.tags.contains(&"#folder/level1d".to_string()));
    assert!(g.tags.contains(&"#folder/level2c".to_string()));
    assert!(g.tags.contains(&"#folder/level3a".to_string()));
}

#[test]
fn parses_nested_collection_root_only() {
    let bytes = read_fixture("nested.html");
    let parsed = parse(&bytes).unwrap();
    // F is in Level1C only (depth 1).
    let f = parsed
        .bookmarks
        .iter()
        .find(|b| b.original_url == "https://f.example.com/")
        .unwrap();
    assert_eq!(f.collection.as_deref(), Some("Root/Level1C"));
}

#[test]
fn parses_tags_attribute_lowercase_sorted() {
    let bytes = read_fixture("with-tags.html");
    let parsed = parse(&bytes).unwrap();
    let b = parsed
        .bookmarks
        .iter()
        .find(|b| b.original_url == "https://rust-lang.org/")
        .unwrap();
    assert!(b.tags.iter().any(|t| t == "rust"));
    assert!(b.tags.iter().any(|t| t == "programming"));
    assert!(b.tags.iter().any(|t| t == "systems"));
    // Sorted ascending.
    let mut sorted = b.tags.clone();
    sorted.sort();
    assert_eq!(b.tags, sorted);
}

#[test]
fn parses_tagged_four_entries() {
    let bytes = read_fixture("with-tags.html");
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.bookmarks.len(), 4);
    // Every entry should have at least one explicit tag plus the folder tag.
    for b in &parsed.bookmarks {
        assert!(!b.tags.is_empty(), "tags empty for {}", b.original_url);
    }
}

#[test]
fn decodes_html_entities_in_title() {
    let bytes = read_fixture("with-special-chars.html");
    let parsed = parse(&bytes).unwrap();
    // First entry title contains entities: AT&T <hello> "world" 'apos'.
    let b = &parsed.bookmarks[0];
    assert!(b.title.contains('&'));
    assert!(b.title.contains('<'));
    assert!(b.title.contains('>'));
    assert!(b.title.contains('"'));
    assert!(b.title.contains('\''));
    // URL keeps the &amp; as & after canonicalize.
    assert!(b.original_url.contains('&'));
}

#[test]
fn decodes_utf8_title() {
    let bytes = read_fixture("with-special-chars.html");
    let parsed = parse(&bytes).unwrap();
    let b = parsed
        .bookmarks
        .iter()
        .find(|b| b.original_url.contains("ja.wikipedia"))
        .unwrap();
    assert!(b.title.contains('日'));
    assert!(b.title.contains('本'));
}

#[test]
fn parses_dd_descriptions() {
    let bytes = read_fixture("with-descriptions.html");
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.bookmarks.len(), 4);
    let alpha = &parsed.bookmarks[0];
    assert_eq!(
        alpha.description.as_deref(),
        Some("First description — useful note about A.")
    );
    // Beta has multiline.
    let beta = &parsed.bookmarks[1];
    assert!(beta
        .description
        .as_deref()
        .unwrap_or("")
        .contains("Second description"));
    // Gamma has no description.
    let gamma = &parsed.bookmarks[2];
    assert!(gamma.description.is_none());
}

#[test]
fn parses_empty_html_safely() {
    let bytes = read_fixture("empty.html");
    let parsed = parse(&bytes).unwrap();
    assert!(parsed.bookmarks.is_empty());
    assert!(parsed.errors.is_empty());
}

#[test]
fn parses_pinboard_export() {
    let bytes = read_fixture("pinboard-export.html");
    let parsed = parse(&bytes).unwrap();
    assert_eq!(parsed.bookmarks.len(), 9);
    // Three folders: reading, tools, unread.
    let cols: std::collections::BTreeSet<String> = parsed
        .bookmarks
        .iter()
        .filter_map(|b| b.collection.clone())
        .collect();
    assert!(cols.contains("reading"));
    assert!(cols.contains("tools"));
    assert!(cols.contains("unread"));
}

#[test]
fn source_impl_open_and_list() {
    let path = fixture("minimal.html");
    let src = NetscapeSource::open(&path).unwrap();
    assert_eq!(src.kind(), linkmarks_core::SourceKind::Netscape);
    let list = src.list().unwrap();
    assert_eq!(list.len(), 3);
}

#[test]
fn source_impl_by_canonical_lookup() {
    let path = fixture("with-tags.html");
    let src = NetscapeSource::open(&path).unwrap();
    let b = src
        .by_canonical("https://crates.io/")
        .unwrap()
        .expect("must exist");
    assert_eq!(b.title, "Crates.io");
}

#[test]
fn source_impl_paginated_returns_some() {
    let path = fixture("nested.html");
    let src = NetscapeSource::open(&path).unwrap();
    let page = src.list_paginated(None, 4).unwrap();
    assert_eq!(page.items.len(), 4);
}

#[test]
fn source_impl_open_missing_file_errors() {
    let res = NetscapeSource::open(std::path::Path::new("/no/such/bookmarks.html"));
    assert!(res.is_err());
}
