//! Integration tests for the Chromium bridge.
//!
//! Runs against the bundled anonymized fixture (5 bookmarks across 3
//! folders, including a tracking-param URL, an HTTPS host with
//! default port, a fragment, and a duplicate canonical URL).

use std::path::PathBuf;

use linkmarks_bridge_chromium::{parse_and_flatten, ChromiumSource};
use linkmarks_core::traits::BookmarkSource;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chrome-bookmarks.example.json")
}

#[test]
fn fixture_exists() {
    let p = fixture_path();
    assert!(p.exists(), "fixture missing: {}", p.display());
}

#[test]
fn parses_fixture_into_six_bookmarks() {
    let (bookmarks, errors) = parse_and_flatten(&fixture_path()).unwrap();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert_eq!(bookmarks.len(), 6, "got bookmarks: {bookmarks:#?}");
}

#[test]
fn canonicalization_strips_tracking_and_port() {
    let (bookmarks, _) = parse_and_flatten(&fixture_path()).unwrap();
    for b in &bookmarks {
        // The fixture includes URL variants that should collapse to
        // the same canonical form; verify no tracking params and no
        // default ports survive.
        assert!(
            !b.canonical_url.contains("utm_"),
            "leaked utm: {}",
            b.canonical_url
        );
        assert!(
            !b.canonical_url.contains("ref="),
            "leaked ref: {}",
            b.canonical_url
        );
        assert!(
            !b.canonical_url.contains("fbclid"),
            "leaked fbclid: {}",
            b.canonical_url
        );
        assert!(
            !b.canonical_url.contains(":443"),
            "leaked :443: {}",
            b.canonical_url
        );
        assert!(
            !b.canonical_url.contains('#'),
            "leaked fragment: {}",
            b.canonical_url
        );
    }
}

#[test]
fn duplicate_canonical_url_detected_by_caller() {
    // The fixture intentionally contains two URLs that canonicalize
    // to the same form. The dedupe step (in core) detects them; this
    // test just verifies the parser produced two records with the
    // same canonical_url.
    let (bookmarks, _) = parse_and_flatten(&fixture_path()).unwrap();
    let mut canonical_urls: Vec<&str> =
        bookmarks.iter().map(|b| b.canonical_url.as_str()).collect();
    canonical_urls.sort();
    let mut dups = Vec::new();
    for pair in canonical_urls.windows(2) {
        if pair[0] == pair[1] {
            dups.push(pair[0]);
        }
    }
    assert!(
        !dups.is_empty(),
        "expected at least one duplicate canonical URL in fixture"
    );
}

#[test]
fn chromium_source_lists_bookmarks() {
    let src = ChromiumSource::open(&fixture_path()).unwrap();
    let bookmarks = src.list().unwrap();
    assert_eq!(bookmarks.len(), 6);
    assert_eq!(src.kind(), linkmarks_core::SourceKind::Chromium);
}

#[test]
fn chromium_source_by_canonical() {
    let src = ChromiumSource::open(&fixture_path()).unwrap();
    let all = src.list().unwrap();
    let some_canonical = &all[0].canonical_url;
    let found = src.by_canonical(some_canonical).unwrap();
    assert!(
        found.is_some(),
        "expected to find a bookmark by canonical URL"
    );
}
