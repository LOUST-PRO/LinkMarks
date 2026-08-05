use linkmarks_bridge_firefox::{parse_jsonlz4_file, FirefoxSource};
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/bookmarks-backups/example.jsonlz4")
}

#[test]
fn jsonlz4_decompresses_and_flattens() {
    let tree = parse_jsonlz4_file(&path()).unwrap();
    assert_eq!(tree.guid, "root________");
    let bookmarks = tree.flatten();
    assert_eq!(bookmarks.len(), 3);
    let tech = bookmarks.iter().find(|b| b.title == "Rust").unwrap();
    assert_eq!(tech.collection.as_deref(), Some("Bookmarks Menu/Tech"));
    assert!(tech.tags.contains(&"rust".to_string()));
    assert!(tech.tags.contains(&"#folder/bookmarks-menu".to_string()));
}

#[test]
fn source_reads_jsonlz4() {
    let source = FirefoxSource::from_jsonlz4_path(path()).unwrap();
    assert_eq!(source.list().unwrap().len(), 3);
}
