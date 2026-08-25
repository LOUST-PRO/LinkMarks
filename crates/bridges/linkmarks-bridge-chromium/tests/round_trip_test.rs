//! Round-trip tests: parse Chromium Bookmarks JSON → emit via
//! `ChromiumSink` → re-parse with `parse_file` → assert that the
//! bookmark set is preserved (by canonical URL + title).

use linkmarks_bridge_chromium::sink::{ChromiumSink, ChromiumTreeFlatten};
use linkmarks_bridge_chromium::parser::{parse_file, parse_and_flatten};
use std::collections::BTreeSet;
use std::path::Path;
use tempfile::tempdir;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct BookmarkKey {
    canonical_url: String,
    title: String,
    collection: String,
}

fn keys(bookmarks: &[linkmarks_core::Bookmark]) -> BTreeSet<BookmarkKey> {
    bookmarks
        .iter()
        .map(|b| BookmarkKey {
            canonical_url: b.canonical_url.clone(),
            title: b.title.clone(),
            collection: b.collection.clone().unwrap_or_default(),
        })
        .collect()
}

fn write_and_reparse(
    input: &Path,
    output: &Path,
) -> (BTreeSet<BookmarkKey>, BTreeSet<BookmarkKey>) {
    let (before, errors) = parse_and_flatten(input).expect("parse input");
    assert!(errors.is_empty(), "input parse errors: {errors:?}");
    let (_, _body) =
        ChromiumSink::write_to(output, &before).expect("write round-trip file");
    let (after, errors) = parse_and_flatten(output).expect("parse output");
    assert!(errors.is_empty(), "output parse errors: {errors:?}");
    (keys(&before), keys(&after))
}

#[test]
fn round_trip_simple() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    let output = dir.path().join("output.json");
    std::fs::write(
        &input,
        r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "Bookmarks bar",
                    "children": [
                        {"type": "url", "name": "Example", "url": "https://example.com/"},
                        {"type": "url", "name": "Google", "url": "https://google.com/"}
                    ]
                },
                "other": {"type": "folder", "name": "Other", "children": []}
            }
        }"#,
    )
    .unwrap();

    let (before, after) = write_and_reparse(&input, &output);
    assert_eq!(before.len(), 2);
    assert_eq!(
        before, after,
        "round-trip should preserve every bookmark's (url, title, collection)"
    );
}

#[test]
fn round_trip_nested_folders() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    let output = dir.path().join("output.json");
    std::fs::write(
        &input,
        r#"{
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
                                    "type": "folder",
                                    "name": "Research",
                                    "children": [
                                        {"type": "url", "name": "Papers", "url": "https://papers.example.com/"},
                                        {"type": "url", "name": "Preprints", "url": "https://preprints.example.com/"}
                                    ]
                                },
                                {"type": "url", "name": "Calendar", "url": "https://cal.example.com/"}
                            ]
                        }
                    ]
                },
                "other": {"type": "folder", "name": "Other", "children": []}
            }
        }"#,
    )
    .unwrap();

    let (before, after) = write_and_reparse(&input, &output);
    assert_eq!(before.len(), 3);
    assert_eq!(before, after);

    // The collection path must survive the round-trip.
    assert!(before.iter().any(|k| k.collection == "Bookmarks bar/Work/Research"));
    assert!(before.iter().any(|k| k.collection == "Bookmarks bar/Work"));
}

#[test]
fn round_trip_preserves_collection_path() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    let output = dir.path().join("output.json");
    std::fs::write(
        &input,
        r#"{
            "roots": {
                "bookmark_bar": {
                    "type": "folder",
                    "name": "Bookmarks bar",
                    "children": [
                        {
                            "type": "folder",
                            "name": "Engineering",
                            "children": [
                                {
                                    "type": "folder",
                                    "name": "Backend",
                                    "children": [
                                        {"type": "url", "name": "GitHub", "url": "https://github.com/"}
                                    ]
                                }
                            ]
                        }
                    ]
                },
                "other": {"type": "folder", "name": "Other", "children": []}
            }
        }"#,
    )
    .unwrap();

    let (before, after) = write_and_reparse(&input, &output);
    assert_eq!(before.len(), 1);
    let key = before.iter().next().unwrap();
    assert_eq!(key.collection, "Bookmarks bar/Engineering/Backend");
    assert_eq!(key.title, "GitHub");
    assert_eq!(key.canonical_url, "https://github.com/");
    assert_eq!(
        before, after,
        "deeply nested collection paths must survive the round-trip"
    );
}

#[test]
fn round_trip_handles_bookmarks_without_collection() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    let output = dir.path().join("output.json");
    std::fs::write(
        &input,
        r#"{
            "roots": {
                "bookmark_bar": {"type": "folder", "name": "Bookmarks bar", "children": []},
                "other": {
                    "type": "folder",
                    "name": "Other bookmarks",
                    "children": [
                        {"type": "url", "name": "ReadLater1", "url": "https://readlater.example.com/1"},
                        {"type": "url", "name": "ReadLater2", "url": "https://readlater.example.com/2"}
                    ]
                }
            }
        }"#,
    )
    .unwrap();

    let (before, after) = write_and_reparse(&input, &output);
    assert_eq!(before.len(), 2);
    // Both bookmarks should be tagged as direct children of `other`
    // — the parser writes `Other bookmarks` (the root's name) as the
    // prefix for any URL nested directly under that root.
    for k in &before {
        assert!(
            k.collection == "Other bookmarks",
            "bookmark in `other` should round-trip with collection=Other bookmarks, got: {:?}",
            k
        );
    }
    assert_eq!(before, after);
}

#[test]
fn chromium_tree_flatten_roundtrips_against_sink() {
    // Build a synthetic tree by hand → flatten → emit via sink →
    // reparse → flatten again. Both flat sets must match.
    use linkmarks_bridge_chromium::parser::{
        BookmarkNode, ChromiumBookmarks, Roots,
    };

    let dir = tempdir().unwrap();
    let output = dir.path().join("output.json");

    let bar_children = vec![BookmarkNode {
        kind: "folder".to_string(),
        name: "Foo".to_string(),
        url: None,
        children: vec![BookmarkNode {
            kind: "url".to_string(),
            name: "A".to_string(),
            url: Some("https://a.example/".to_string()),
            children: Vec::new(),
            date_added: None,
            date_last_used: None,
        }],
        date_added: None,
        date_last_used: None,
    }];

    let tree = ChromiumBookmarks {
        roots: Roots {
            bookmark_bar: BookmarkNode {
                kind: "folder".to_string(),
                name: "Bookmarks bar".to_string(),
                url: None,
                children: bar_children,
                date_added: None,
                date_last_used: None,
            },
            other: BookmarkNode {
                kind: "folder".to_string(),
                name: "Other bookmarks".to_string(),
                url: None,
                children: Vec::new(),
                date_added: None,
                date_last_used: None,
            },
            synced: None,
            custom_root: None,
        },
    };

    let bookmarks = tree.into_flat_bookmarks();
    assert_eq!(bookmarks.len(), 1);
    ChromiumSink::write_to(&output, &bookmarks).expect("write sink");

    let reparsed = parse_file(&output).expect("reparse");
    let after = reparsed.into_flat_bookmarks();
    assert_eq!(
        keys(&bookmarks),
        keys(&after),
        "sink + parser must compose: bookmarks emitted by the sink should parse back to the same set"
    );
}
