//! `yrs-bench` — yrs smoke / dep-path check.
//!
//! Loads the 10k-bookmark fixture, instantiates a `yrs::Doc`, and
//! exits. This binary exists to keep `yrs` in the workspace dependency
//! graph so the heavier encode/concurrent suites can call into it.
//! Real measurements live in `compare`, `compare-concurrent`, and the
//! HTTP roundtrip pair.

use linkmarks_bench_crdt::fixture;

fn main() {
    eprintln!("=== yrs-bench (smoke) ===");
    eprintln!("yrs crate-version available via `cargo tree -p yrs`");

    eprintln!("Loading standard fixture (10k synthetic bookmarks)…");
    let bookmarks = fixture::standard_fixture();
    eprintln!("Loaded {} bookmarks.", bookmarks.len());

    let first = &bookmarks[0];
    eprintln!(
        "Sample[0]: id={} url={} tags={:?} source={:?}",
        first.id, first.original_url, first.tags, first.source
    );

    // Touch the yrs API surface so `cargo build` exercises the dep path.
    let _doc = yrs::Doc::new();

    eprintln!("yrs-bench OK.");
}
