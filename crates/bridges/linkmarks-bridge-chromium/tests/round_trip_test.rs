//! Round-trip tests: parse Chromium Bookmarks JSON → emit via
//! `ChromiumSink` → re-parse with `parse_file` → assert that the
//! bookmark set is preserved (by canonical URL + title).

use linkmarks_bridge_chromium::parser::{parse_and_flatten, parse_file};
use linkmarks_bridge_chromium::sink::{ChromiumSink, ChromiumTreeFlatten};
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
    let (_, _body) = ChromiumSink::write_to(output, &before).expect("write round-trip file");
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
    assert!(before
        .iter()
        .any(|k| k.collection == "Bookmarks bar/Work/Research"));
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
fn round_trip_preserves_unicode_titles() {
    // Chromium Bookmarks JSON is UTF-8. Titles with non-ASCII
    // characters (CJK, emoji, diacritics) must round-trip byte-exact
    // through parse → write → parse. The sink uses `serde_json`
    // (which is UTF-8-native) so the failure mode would be silent
    // truncation if any layer accidentally downcast to `&str` and
    // assumed ASCII. This test guards against that.
    let unicode_titles = [
        ("Japanese", "https://jp.example.com/", "日本"),
        ("Emoji", "https://emoji.example.com/", "🦀🚀"),
        (
            "Diacritics",
            "https://diacritics.example.com/",
            "café résumé",
        ),
        ("Mixed RTL", "https://rtl.example.com/", "עברית"),
        ("Zero-width joiner", "https://zwj.example.com/", "👨‍👩‍👧‍👦"),
    ];
    let children_json: String = unicode_titles
        .iter()
        .map(|(_, url, title)| {
            format!(
                r#"{{"type": "url", "name": {title_json}, "url": "{url}"}}"#,
                title_json = serde_json::to_string(title).unwrap(),
                url = url,
            )
        })
        .collect::<Vec<_>>()
        .join(",\n                    ");

    let dir = tempdir().unwrap();
    let input = dir.path().join("input.json");
    let output = dir.path().join("output.json");
    std::fs::write(
        &input,
        format!(
            r#"{{
                "roots": {{
                    "bookmark_bar": {{
                        "type": "folder",
                        "name": "Bookmarks bar",
                        "children": [
                            {children}
                        ]
                    }},
                    "other": {{"type": "folder", "name": "Other", "children": []}}
                }}
            }}"#,
            children = children_json,
        ),
    )
    .unwrap();

    let (before, after) = write_and_reparse(&input, &output);
    assert_eq!(before.len(), unicode_titles.len());
    assert_eq!(
        before, after,
        "unicode titles must survive parse → write → parse byte-exact"
    );

    // Spot-check the actual UTF-8 byte content: emit a bookmark with
    // an emoji title, write it via the sink, and read the raw bytes
    // back to confirm the byte sequence is preserved (not escaped,
    // not normalized, not truncated).
    let emoji_bm = linkmarks_core::Bookmark {
        id: linkmarks_core::BookmarkId::generate(),
        original_url: "https://emoji.example/".to_string(),
        canonical_url: "https://emoji.example/".to_string(),
        title: "🚀 test 🚀".to_string(),
        description: None,
        tags: Vec::new(),
        collection: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        source: linkmarks_core::SourceRef {
            kind: linkmarks_core::SourceKind::Chromium,
            external_id: None,
            imported_at: chrono::Utc::now(),
            raw: None,
        },
        content_type: None,
        archived: false,
    };
    let (_, body) = ChromiumSink::write_to(&output, std::slice::from_ref(&emoji_bm)).unwrap();
    assert!(
        body.contains("🚀 test 🚀"),
        "emoji bytes must appear verbatim in the rendered JSON"
    );
    // Verify the byte sequence is the canonical UTF-8 for 🚀
    // (U+1F680 = F0 9F 9A 80). If serde escaped it, it would be
    // `🚀` instead.
    assert!(
        body.contains("\u{1F680}"),
        "emoji must be encoded as raw UTF-8, not as a \\uXXXX escape"
    );
    assert!(
        !body.contains("\\ud83d\\ude80"),
        "serde must not escape emoji to surrogate-pair form"
    );
}

#[test]
fn write_does_not_corrupt_concurrent_reader() {
    // File-based write-back (vs live write-back) means the user
    // imports the produced JSON via Vivaldi's "Import bookmarks"
    // UI. The contract: a reader holding the file open during a
    // `write_to` call sees either the previous content or the new
    // content — never a truncated or interleaved byte sequence.
    // The sink uses `tempfile::NamedTempFile::persist` (atomic
    // rename), so the rename swaps the inode in one syscall. This
    // test pins the contract so any future switch to in-place write
    // triggers a deliberate review.
    let dir = tempdir().unwrap();
    let target = dir.path().join("Bookmarks");

    // Initial content: 1 bookmark.
    let initial = bk("https://initial.example/", "Initial", None);
    let (_, _) = ChromiumSink::write_to(&target, &[initial]).unwrap();

    // Open the file for reading (simulates the browser's handle).
    let reader = std::fs::File::open(&target).unwrap();
    let initial_bytes = std::fs::read(&target).unwrap();
    assert!(!initial_bytes.is_empty());

    // While the reader holds the file, write new content.
    let updated = bk("https://updated.example/", "Updated", None);
    let (_, _) = ChromiumSink::write_to(&target, &[updated]).unwrap();

    // After the write, the file on disk must contain the new
    // content (not truncated, not interleaved). The reader's
    // previously-opened FD still refers to the old inode on most
    // platforms; we read the on-disk file to verify the new bytes.
    let post_write_bytes = std::fs::read(&target).unwrap();
    let post_write_str = std::str::from_utf8(&post_write_bytes).unwrap();
    assert!(
        post_write_str.contains("https://updated.example/"),
        "after write, on-disk file must contain new content"
    );
    assert!(
        !post_write_str.contains("https://initial.example/"),
        "after write, on-disk file must not still contain the old URL"
    );

    // The reader's FD still points to the pre-rename inode; reading
    // from it is OS-dependent (Linux gives the old content until
    // the FD is closed, Windows may invalidate). We don't assert on
    // the reader's view — only on the on-disk file's integrity.
    drop(reader);
}

fn bk(url: &str, title: &str, collection: Option<&str>) -> linkmarks_core::Bookmark {
    linkmarks_core::Bookmark {
        id: linkmarks_core::BookmarkId::generate(),
        original_url: url.to_string(),
        canonical_url: url.to_string(),
        title: title.to_string(),
        description: None,
        tags: Vec::new(),
        collection: collection.map(str::to_string),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        source: linkmarks_core::SourceRef {
            kind: linkmarks_core::SourceKind::Chromium,
            external_id: None,
            imported_at: chrono::Utc::now(),
            raw: None,
        },
        content_type: None,
        archived: false,
    }
}

#[test]
fn chromium_tree_flatten_roundtrips_against_sink() {
    // Build a synthetic tree by hand → flatten → emit via sink →
    // reparse → flatten again. Both flat sets must match.
    use linkmarks_bridge_chromium::parser::{BookmarkNode, ChromiumBookmarks, Roots};

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
