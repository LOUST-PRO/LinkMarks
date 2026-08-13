//! Filter end-to-end: build a small list of bookmarks, run the
//! filter, and verify the same logic the App uses.

use chrono::{TimeZone, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use linkmarks_tui::state::{AppState, FilterMode};

fn bm(title: &str, url: &str, tags: &[&str], collection: Option<&str>) -> Bookmark {
    Bookmark {
        id: BookmarkId::generate(),
        original_url: url.into(),
        canonical_url: url.into(),
        title: title.into(),
        description: None,
        tags: tags.iter().map(|s| s.to_string()).collect(),
        collection: collection.map(|s| s.to_string()),
        created_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        updated_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        source: SourceRef {
            kind: SourceKind::Chromium,
            external_id: None,
            imported_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            raw: None,
        },
        content_type: None,
        archived: false,
    }
}

fn filter(bookmarks: &[Bookmark], query: &str, mode: FilterMode) -> Vec<String> {
    bookmarks
        .iter()
        .filter(|b| AppState::matches(b, query, mode))
        .map(|b| b.title.clone())
        .collect()
}

#[test]
fn empty_query_returns_all() {
    let list = vec![
        bm("a", "https://a", &[], None),
        bm("b", "https://b", &[], None),
    ];
    assert_eq!(filter(&list, "", FilterMode::Substring), vec!["a", "b"]);
    assert_eq!(filter(&list, "   ", FilterMode::Substring), vec!["a", "b"]);
}

#[test]
fn case_insensitive_title_match() {
    let list = vec![
        bm("Hello World", "https://x", &[], None),
        bm("Goodbye", "https://y", &[], None),
    ];
    assert_eq!(filter(&list, "hello", FilterMode::Substring), vec!["Hello World"]);
    assert_eq!(filter(&list, "WORLD", FilterMode::Substring), vec!["Hello World"]);
    assert_eq!(filter(&list, "xyz", FilterMode::Substring), Vec::<String>::new());
}

#[test]
fn url_substring_match() {
    let list = vec![
        bm("a", "https://example.com/foo", &[], None),
        bm("b", "https://other.com/bar", &[], None),
    ];
    assert_eq!(
        filter(&list, "example.com", FilterMode::Substring),
        vec!["a"]
    );
}

#[test]
fn tag_match() {
    let list = vec![
        bm("a", "https://a", &["rust", "cli"], None),
        bm("b", "https://b", &["python"], None),
    ];
    assert_eq!(filter(&list, "rust", FilterMode::Substring), vec!["a"]);
    assert_eq!(filter(&list, "cli", FilterMode::Substring), vec!["a"]);
    assert_eq!(filter(&list, "python", FilterMode::Substring), vec!["b"]);
}

#[test]
fn collection_match() {
    let list = vec![
        bm("a", "https://a", &[], Some("work/projects")),
        bm("b", "https://b", &[], Some("personal")),
    ];
    assert_eq!(filter(&list, "work", FilterMode::Substring), vec!["a"]);
    assert_eq!(filter(&list, "personal", FilterMode::Substring), vec!["b"]);
}

#[test]
fn tag_mode_only_tags() {
    let list = vec![
        bm("Hello world", "https://x", &["rust"], None),
        bm("rusty", "https://y", &[], None),
    ];
    assert_eq!(filter(&list, "rust", FilterMode::Tag), vec!["Hello world"]);
    assert_eq!(filter(&list, "rusty", FilterMode::Tag), Vec::<String>::new());
}

#[test]
fn tag_mode_picks_winning_tag() {
    let list = vec![
        bm("a", "https://a", &["rust", "cli"], None),
        bm("b", "https://b", &["python"], None),
        bm("c", "https://c", &["rust"], None),
    ];
    assert_eq!(filter(&list, "rust", FilterMode::Tag), vec!["a", "c"]);
}
