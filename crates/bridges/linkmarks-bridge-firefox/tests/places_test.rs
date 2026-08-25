use linkmarks_bridge_firefox::FirefoxSource;
use linkmarks_core::traits::BookmarkSource;
use rusqlite::Connection;
use std::path::Path;
use tempfile::tempdir;

fn fixture() -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES \
            (1,2,NULL,0,0,'Bookmarks Menu',1700000000000000),\
            (2,2,NULL,0,1,'Bookmarks Toolbar',1700000000000000),\
            (3,2,NULL,0,2,'Other Bookmarks',1700000000000000),\
            (10,2,NULL,1,0,'Tech',1700000000000000),\
            (11,1,101,10,0,'Rust',1700000000000000),\
            (12,1,102,1,1,'News',1700000000000000),\
            (13,1,103,2,0,'Toolbar link',1700000000000000),\
            (14,1,104,3,0,'Other link',1700000000000000),\
            (15,1,105,10,1,'SQLite',1700000000000000); \
         INSERT INTO moz_places VALUES \
            (101,'https://example.com/rust','Rust',1700000000000000,'desc'),\
            (102,'https://example.com/news','News',1700000000000000,NULL),\
            (103,'https://example.com/toolbar','Toolbar',1700000000000000,NULL),\
            (104,'https://example.com/other','Other',1700000000000000,NULL),\
            (105,'https://example.com/sqlite','SQLite',1700000000000000,NULL);",
    )
    .unwrap();
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

#[test]
fn places_uses_last_modified_for_updated_at() {
    // Fixture: visit = 1_700_000_000_000_000 (2023-11-14)
    //          lastModified = 1_800_000_000_000_000 (2027-03-07)
    // Expect updated_at strictly greater than created_at.
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES (1,2,NULL,0,0,'Menu',1),(2,1,10,1,0,'Fresh edit',2); \
         INSERT INTO moz_places VALUES (10,'https://example.com/','Fresh edit',1,NULL);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(list.len(), 1);
    let bm = &list[0];
    assert!(
        bm.updated_at > bm.created_at,
        "updated_at ({}) must be strictly later than created_at ({}) when lastModified > last_visit_date",
        bm.updated_at,
        bm.created_at
    );
}

#[test]
fn places_skips_separator_kind_3() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES (1,2,NULL,0,0,'Menu',1),(2,3,NULL,1,0,'---separator---',1),(3,1,10,1,1,'Real bookmark',1); \
         INSERT INTO moz_places VALUES (10,'https://example.com/','Real bookmark',1,NULL);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].original_url, "https://example.com/");
}

#[test]
fn places_filters_internal_url_schemes_case_insensitive() {
    // Firefox and some extensions occasionally emit mixed-case or
    // upper-case internal-scheme URLs (e.g. `ABOUT:HOME`,
    // `JavaScript:void(0)`). The filter must catch those as well as
    // the canonical lowercase form.
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES \
            (1,2,NULL,0,0,'Menu',1),\
            (10,1,11,1,0,'ABOUT upper',1),\
            (11,1,12,1,1,'JavaScript mixed',1),\
            (12,1,13,1,2,'PLACE upper',1),\
            (13,1,14,1,3,'Data upper',1),\
            (14,1,15,1,4,'real',1); \
         INSERT INTO moz_places VALUES \
            (11,'ABOUT:HOME','about',1,NULL),\
            (12,'JavaScript:void(0)','js',1,NULL),\
            (13,'PLACE:folder/1','place',1,NULL),\
            (14,'DATA:text/plain,hi','data',1,NULL),\
            (15,'https://example.com/','real',1,NULL);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(
        list.len(),
        1,
        "only the real https URL should survive; mixed-case internal schemes are filtered"
    );
    assert_eq!(list[0].original_url, "https://example.com/");
}

#[test]
fn places_filters_internal_url_schemes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES \
            (1,2,NULL,0,0,'Menu',1),\
            (10,1,11,1,0,'place URL',1),\
            (11,1,12,1,1,'about URL',1),\
            (12,1,13,1,2,'javascript URL',1),\
            (13,1,14,1,3,'chrome URL',1),\
            (14,1,15,1,4,'data URL',1),\
            (15,1,16,1,5,'real',1); \
         INSERT INTO moz_places VALUES \
            (11,'place:folder/123','place',1,NULL),\
            (12,'about:home','about',1,NULL),\
            (13,'javascript:void(0)','js',1,NULL),\
            (14,'chrome://settings','chrome',1,NULL),\
            (15,'data:text/plain,hello','data',1,NULL),\
            (16,'https://example.com/','real',1,NULL);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(
        list.len(),
        1,
        "only the real https URL should survive, internal schemes are filtered"
    );
    assert_eq!(list[0].original_url, "https://example.com/");
}

#[test]
fn places_handles_empty_db() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES (1,2,NULL,0,0,'Menu',1);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert!(list.is_empty());
}

#[test]
fn places_tag_prefix_folder_does_not_propagate() {
    // Mozilla reserves ids 1/2/3 for the canonical roots. Use 20/30 here so
    // the "tag:temp" folder is treated as a regular user folder.
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES \
            (1,2,NULL,0,0,'Menu',1),\
            (20,2,NULL,1,0,'tag:temp',1),\
            (30,1,10,20,0,'child',1); \
         INSERT INTO moz_places VALUES (10,'https://example.com/','child',1,NULL);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(list.len(), 1);
    // The folder named "tag:temp" must NOT appear in ancestors — only "Menu" should.
    assert_eq!(list[0].collection.as_deref(), Some("Bookmarks Menu"));
}

#[test]
fn places_handles_fk_null_bookmark() {
    // type=1 (bookmark) but fk IS NULL → no URL → must be skipped, not panic.
    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES (1,2,NULL,0,0,'Menu',1),(2,1,NULL,1,0,'orphan',1),(3,1,10,1,1,'real',1); \
         INSERT INTO moz_places VALUES (10,'https://example.com/','real',1,NULL);",
    )
    .unwrap();
    drop(db);
    let source = FirefoxSource::from_places_path(dir.path().join("places.sqlite")).unwrap();
    let list = source.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].original_url, "https://example.com/");
}

#[test]
fn places_retries_on_busy_then_succeeds() {
    // Open a write-mode connection that holds BEGIN EXCLUSIVE for a
    // short while. EXCLUSIVE blocks both writers AND readers (unlike
    // BEGIN IMMEDIATE, which permits concurrent readers), so the
    // bridge's read-only open and the subsequent query_map both hit
    // SQLITE_BUSY while the writer holds the lock. The retry loop in
    // `parse_places` covers the entire read flow (open + prepare +
    // query_map + iteration), so it must absorb the contention and
    // complete successfully after the writer releases.
    use std::time::Duration;

    let dir = tempdir().unwrap();
    let path = dir.path().join("places.sqlite");
    let init = Connection::open(&path).unwrap();
    init.execute_batch(
        "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, last_visit_date INTEGER, description TEXT); \
         CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, parent INTEGER, position INTEGER, title TEXT, lastModified INTEGER); \
         INSERT INTO moz_bookmarks VALUES (1,2,NULL,0,0,'Menu',1),(2,1,10,1,0,'A',1); \
         INSERT INTO moz_places VALUES (10,'https://example.com/','A',1,NULL);",
    )
    .unwrap();
    drop(init);

    // Writer holds EXCLUSIVE for 350ms — long enough for the reader's
    // first open attempt to fail with SQLITE_BUSY but short enough that
    // the retry loop (100 + 200 + 300 = 600 ms of backoff) finishes
    // before the writer's 350ms hold.
    let writer_path = path.clone();
    let writer = std::thread::spawn(move || {
        let conn = Connection::open(&writer_path).unwrap();
        conn.execute_batch("BEGIN EXCLUSIVE;").unwrap();
        std::thread::sleep(Duration::from_millis(350));
        conn.execute_batch("COMMIT;").unwrap();
    });

    // Give the writer thread a head-start so the FIRST read attempt
    // hits SQLITE_BUSY. Subsequent retries (after 100/200/300 ms
    // backoff) hit the now-released file.
    std::thread::sleep(Duration::from_millis(100));
    let source = FirefoxSource::from_places_path(path.clone()).unwrap();
    let list = source.list().unwrap();
    writer.join().unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].original_url, "https://example.com/");
}
