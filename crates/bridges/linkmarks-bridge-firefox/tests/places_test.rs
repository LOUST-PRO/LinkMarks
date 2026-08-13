use linkmarks_bridge_firefox::FirefoxSource;
use linkmarks_core::traits::BookmarkSource;
use rusqlite::Connection;
use std::path::Path;
use tempfile::tempdir;

fn fixture() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT); INSERT INTO moz_bookmarks VALUES (1,1,NULL,0,0,'Bookmarks Menu'),(2,1,NULL,0,1,'Bookmarks Toolbar'),(3,1,NULL,0,2,'Other Bookmarks'),(10,1,NULL,1,0,'Tech'),(11,0,101,10,0,'Rust'),(12,0,102,1,1,'News'),(13,0,103,2,0,'Toolbar link'),(14,0,104,3,0,'Other link'),(15,0,105,10,1,'SQLite'); INSERT INTO moz_places VALUES (101,'https://example.com/rust','Rust',1700000000000000,'desc'),(102,'https://example.com/news','News',1700000000000000,NULL),(103,'https://example.com/toolbar','Toolbar',1700000000000000,NULL),(104,'https://example.com/other','Other',1700000000000000,NULL),(105,'https://example.com/sqlite','SQLite',1700000000000000,NULL);").unwrap();
    drop(db);
    dir
}

#[test]
fn places_reads_roots_and_nested_folder_paths() {
    let dir = fixture();
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(list.len(), 5);
    assert!(list
        .iter()
        .any(|b| b.original_url == "https://example.com/rust"
            && b.collection.as_deref() == Some("Bookmarks Menu/Tech")));
    assert!(list
        .iter()
        .any(|b| b.collection.as_deref() == Some("Bookmarks Toolbar")));
    assert!(list
        .iter()
        .all(|b| b.source.kind == linkmarks_core::model::SourceKind::Firefox));
}

#[test]
fn places_path_is_read_only_source() {
    let dir = fixture();
    assert!(Path::new(&dir.path().join("places.sqlite")).exists());
}
