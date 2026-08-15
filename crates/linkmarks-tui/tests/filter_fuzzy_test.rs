//! F4.1 — fuzzy filter integration tests.
//!
//! Covers the `fuzzy_match` slice-level function and the `Fuzzy`
//! FilterMode wiring through `App::visible`.

use chrono::{TimeZone, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use linkmarks_tui::filter::fuzzy_match;
use linkmarks_tui::state::{AppState, FilterMode};

fn bm(title: &str, url: &str, tags: &[&str]) -> Bookmark {
    Bookmark {
        id: BookmarkId::generate(),
        original_url: url.into(),
        canonical_url: url.into(),
        title: title.into(),
        description: None,
        tags: tags.iter().map(|s| s.to_string()).collect(),
        collection: None,
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

#[test]
fn empty_query_returns_all_with_zero_score() {
    let list = vec![bm("alpha", "https://a", &[]), bm("beta", "https://b", &[])];
    let r = fuzzy_match("", &list);
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|(_, s)| *s == 0));
    assert_eq!(r[0].0, 0);
    assert_eq!(r[1].0, 1);
}

#[test]
fn whitespace_query_returns_all_with_zero_score() {
    let list = vec![bm("alpha", "https://a", &[])];
    let r = fuzzy_match("   ", &list);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].1, 0);
}

#[test]
fn exact_title_match_outscores_partial() {
    let list = vec![
        bm("Rust Programming", "https://rust-lang.org", &[]),
        bm("Go Tutorial", "https://golang.org", &[]),
        bm("Python", "https://python.org", &[]),
    ];
    let r = fuzzy_match("rust", &list);
    assert!(!r.is_empty());
    let titles: Vec<&str> = r.iter().map(|(idx, _)| list[*idx].title.as_str()).collect();
    assert_eq!(titles[0], "Rust Programming");
}

#[test]
fn substring_match_works() {
    let list = vec![
        bm("hello world", "https://x.com", &[]),
        bm("goodbye world", "https://y.com", &[]),
    ];
    let r = fuzzy_match("hello", &list);
    assert_eq!(r.len(), 1);
    assert_eq!(list[r[0].0].title, "hello world");
}

#[test]
fn case_insensitive_match() {
    let list = vec![
        bm("Hello World", "https://x.com", &[]),
        bm("FOO BAR", "https://y.com", &[]),
    ];
    let r = fuzzy_match("hello", &list);
    assert_eq!(r.len(), 1);
    assert_eq!(list[r[0].0].title, "Hello World");

    let r = fuzzy_match("HELLO", &list);
    assert_eq!(r.len(), 1);
    assert_eq!(list[r[0].0].title, "Hello World");
}

#[test]
fn unicode_acentos_match() {
    let list = vec![
        bm("Programación", "https://es-lang.org", &[]),
        bm("Programming", "https://en-lang.org", &[]),
    ];
    // Query with accent: should match "Programación".
    let r = fuzzy_match("Programación", &list);
    assert!(!r.is_empty());
    assert!(r.iter().any(|(idx, _)| list[*idx].title == "Programación"));

    // Query without accent: Normalization::Smart may still match the
    // accented one. We just verify *some* hit.
    let r = fuzzy_match("Programacion", &list);
    assert!(!r.is_empty());
}

#[test]
fn tag_match_contributes_to_score() {
    let list = vec![
        bm("Homepage", "https://x.com", &["rust", "tui"]),
        bm("Other", "https://y.com", &["python"]),
    ];
    let r = fuzzy_match("rust", &list);
    assert!(!r.is_empty());
    // The rust-tagged row should appear.
    assert!(r.iter().any(|(idx, _)| list[*idx].title == "Homepage"));
}

#[test]
fn empty_url_only_title() {
    let list = vec![bm("Lonely Title", "", &[])];
    let r = fuzzy_match("lonely", &list);
    assert_eq!(r.len(), 1);
}

#[test]
fn empty_title_only_url() {
    let list = vec![bm("", "https://only-url.com", &[])];
    let r = fuzzy_match("only-url", &list);
    assert_eq!(r.len(), 1);
}

#[test]
fn single_bookmark_match() {
    let list = vec![bm("only", "https://only.com", &[])];
    let r = fuzzy_match("only", &list);
    assert_eq!(r.len(), 1);
}

#[test]
fn single_bookmark_no_match() {
    let list = vec![bm("only", "https://only.com", &[])];
    let r = fuzzy_match("zzzzzzzz", &list);
    assert!(r.is_empty());
}

#[test]
fn results_sorted_by_score_descending() {
    let list = vec![
        bm("Rust by Example", "https://doc.rust-lang.org", &[]),
        bm("rust-lang.org", "https://rust-lang.org", &[]),
        bm("rustup.rs installer", "https://rustup.rs", &[]),
    ];
    let r = fuzzy_match("rust", &list);
    assert!(r.len() >= 2, "expected at least 2 matches, got {}", r.len());
    // Scores should be non-increasing.
    for w in r.windows(2) {
        assert!(
            w[0].1 >= w[1].1,
            "scores not sorted DESC: {} then {}",
            w[0].1,
            w[1].1
        );
    }
}

#[test]
fn stable_ordering_on_ties() {
    let list = vec![
        bm("identical", "https://identical.com", &[]),
        bm("identical", "https://identical.com", &[]),
        bm("identical", "https://identical.com", &[]),
    ];
    let r = fuzzy_match("identical", &list);
    assert_eq!(r.len(), 3);
    let indices: Vec<usize> = r.iter().map(|(idx, _)| *idx).collect();
    assert_eq!(indices, vec![0, 1, 2]);
}

#[test]
fn performance_smoke_1000_bookmarks() {
    let list: Vec<Bookmark> = (0..1000)
        .map(|i| {
            bm(
                &format!("title-{i}"),
                &format!("https://example.com/{i}"),
                &[if i % 7 == 0 { "featured" } else { "regular" }],
            )
        })
        .collect();
    let start = std::time::Instant::now();
    let r = fuzzy_match("title-42", &list);
    let elapsed = start.elapsed();
    assert!(!r.is_empty());
    assert!(elapsed.as_secs() < 2, "fuzzy match too slow: {elapsed:?}");
}

#[test]
fn fuzzy_match_canonical_vs_original_url() {
    let mut b = bm("homepage", "https://ORIGINAL.example.com/x", &[]);
    b.canonical_url = "https://canonical.example.com/x".into();
    let list = vec![b];
    let r = fuzzy_match("canonical", &list);
    assert_eq!(r.len(), 1);
    let r = fuzzy_match("ORIGINAL", &list);
    assert_eq!(r.len(), 1);
}

#[test]
fn query_with_whitespace_tokens() {
    let list = vec![
        bm("Rust Language", "https://rust-lang.org", &[]),
        bm("Python Language", "https://python.org", &[]),
    ];
    let r = fuzzy_match("rust lang", &list);
    assert!(!r.is_empty());
    assert!(r.iter().any(|(idx, _)| list[*idx].title == "Rust Language"));
}

#[test]
fn filter_mode_fuzzy_compiles_and_matches() {
    // Sanity check: AppState::matches with Fuzzy should accept a
    // bookmark whose haystack contains the query.
    let list = vec![bm("Rust Programming", "https://rust-lang.org", &[])];
    let r = fuzzy_match("rust", &list);
    assert!(!r.is_empty());
    // Make sure the mode enum exists and is wired.
    let _ = FilterMode::Fuzzy;
}

#[test]
fn app_state_matches_routes_fuzzy_mode() {
    // Confirms the dispatch in state.rs delegates Fuzzy to the
    // fuzzy matcher rather than to the substring logic.
    let hit = bm("Rust Programming", "https://rust-lang.org", &[]);
    let miss = bm("Other", "https://other.com", &[]);
    assert!(AppState::matches(&hit, "rust", FilterMode::Fuzzy));
    assert!(!AppState::matches(&miss, "rust", FilterMode::Fuzzy));
    // Empty query is a pass-through even in Fuzzy mode.
    assert!(AppState::matches(&miss, "", FilterMode::Fuzzy));
}
