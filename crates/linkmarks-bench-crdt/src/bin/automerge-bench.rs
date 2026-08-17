//! `automerge-bench` — automerge smoke / dep-path check.
//!
//! Loads the 10k-bookmark fixture, instantiates an `automerge::Automerge`
//! doc, and exits. This binary exists to keep `automerge` in the workspace
//! dependency graph so the heavier encode/concurrent suites can call into
//! it. Real measurements live in `compare`, `compare-concurrent`, and the
//! HTTP roundtrip pair.

use linkmarks_bench_crdt::fixture;

fn main() {
    eprintln!("=== automerge-bench (smoke) ===");
    eprintln!(
        "automerge crate-version available via `cargo tree -p automerge`"
    );

    eprintln!("Loading standard fixture (10k synthetic bookmarks)…");
    let bookmarks = fixture::standard_fixture();
    eprintln!("Loaded {} bookmarks.", bookmarks.len());

    let last = &bookmarks[bookmarks.len() - 1];
    eprintln!(
        "Sample[last]: id={} url={} source={:?}",
        last.id, last.original_url, last.source
    );

    // Touch the automerge API surface so `cargo build` exercises the
    // dep path.
    let mut _doc = automerge::Automerge::new();

    eprintln!("automerge-bench OK.");
}
