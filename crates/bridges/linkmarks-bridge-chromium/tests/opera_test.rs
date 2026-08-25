#[test]
fn parses_opera_gx_real_bookmarks() {
    let path = std::path::Path::new("/home/lou/.config/opera-gx/Default/Bookmarks");
    if !path.exists() {
        eprintln!("SKIP: opera-gx Bookmarks not present");
        return;
    }
    let (bookmarks, errors) = linkmarks_bridge_chromium::parser::parse_and_flatten(path)
        .expect("parse should succeed");
    eprintln!("OPERA-GX: bookmarks={} errors={}", bookmarks.len(), errors.len());
    for e in &errors {
        eprintln!("  ERR: {e:?}");
    }
    for b in bookmarks.iter().take(5) {
        eprintln!("  BM: title={:?} url={:?} coll={:?}",
            b.title, b.canonical_url, b.collection);
    }
    assert!(!bookmarks.is_empty(), "opera-gx should produce at least 1 bookmark");
}
