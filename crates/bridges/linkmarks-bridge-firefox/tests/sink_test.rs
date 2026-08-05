use chrono::{TimeZone, Utc};
use linkmarks_bridge_firefox::FirefoxJsonSink;
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use linkmarks_core::traits::BookmarkSink;
use tempfile::tempdir;

fn bookmark(url: &str, collection: &str) -> Bookmark {
    let time = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();
    Bookmark {
        id: BookmarkId::generate(),
        original_url: url.into(),
        canonical_url: url.into(),
        title: "Example".into(),
        description: None,
        tags: vec!["rust".into(), "#folder/ignored".into()],
        collection: Some(collection.into()),
        created_at: time,
        updated_at: time,
        source: SourceRef {
            kind: SourceKind::Firefox,
            external_id: Some("guid".into()),
            imported_at: time,
            raw: None,
        },
        content_type: None,
        archived: false,
    }
}

#[test]
fn sink_writes_round_trippable_firefox_shape() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bookmarks.json");
    let mut sink = FirefoxJsonSink::open(&path);
    let report = sink
        .write(&[bookmark("https://example.com", "Bookmarks Menu/Tech")])
        .unwrap();
    assert_eq!(report.written, 1);
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(value["guid"], "root________");
    assert_eq!(value["typeCode"], 1);
    assert_eq!(value["children"][0]["title"], "Bookmarks Menu");
    assert_eq!(value["children"][0]["children"][0]["title"], "Tech");
    assert_eq!(
        value["children"][0]["children"][0]["children"][0]["uri"],
        "https://example.com"
    );
}
