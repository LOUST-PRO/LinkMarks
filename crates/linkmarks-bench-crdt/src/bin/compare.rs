//! `compare` — encode-comparison driver.
//!
//! Loads the standard 10k-bookmark fixture, runs both `yrs_measure`
//! and `automerge_measure`, and prints a comparison table that backs
//! the perf numbers in `crates/linkmarks-bench-crdt/RESULTS-encode-comparison.md`.
//!
//! Run with `--release` for measurement-quality numbers. The dev
//! profile will compile, but LTO / opt-level differences will skew
//! RSS upward by ~5-15% and encode size downward (more aggressive
//! dead-code elimination).

use linkmarks_bench_crdt::{automerge_measure, fixture, yrs_measure};

fn human_bytes(b: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let n = b as f64;
    if n >= MB {
        format!("{:.2} MB", n / MB)
    } else if n >= KB {
        format!("{:.2} KB", n / KB)
    } else {
        format!("{b} B")
    }
}

fn human_rss(b: u64) -> String {
    human_bytes(b as usize)
}

fn main() {
    eprintln!("=== compare (encode-comparison suite) ===");
    eprintln!("Loading standard 10k-bookmark fixture…");
    let bookmarks = fixture::standard_fixture();
    eprintln!("Loaded {} bookmarks.", bookmarks.len());

    eprintln!();
    eprintln!("--- yrs v0.20 ---");
    let yrs = yrs_measure::measure(&bookmarks);
    eprintln!(
        "collections: {} | bookmarks: {}",
        yrs.collection_count, yrs.bookmark_count
    );
    for (col, n) in &yrs.per_collection_bytes {
        eprintln!("  {:>16}  {}", col, human_bytes(*n));
    }
    eprintln!("  {:>16}  {}", "TOTAL encode", human_bytes(yrs.total_encoded_bytes));
    eprintln!("  {:>16}  {}", "peak RSS", human_rss(yrs.peak_rss_bytes));

    eprintln!();
    eprintln!("--- automerge v0.5.12 ---");
    let am = match automerge_measure::measure(&bookmarks) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("automerge_measure failed: {e}");
            std::process::exit(2);
        }
    };
    eprintln!(
        "collections: {} | bookmarks: {}",
        am.collection_count, am.bookmark_count
    );
    for (col, n) in &am.per_collection_bytes {
        eprintln!("  {:>16}  {}", col, human_bytes(*n));
    }
    eprintln!("  {:>16}  {}", "TOTAL encode", human_bytes(am.total_encoded_bytes));
    eprintln!("  {:>16}  {}", "peak RSS", human_rss(am.peak_rss_bytes));

    eprintln!();
    eprintln!("--- comparison ---");
    let y = yrs.total_encoded_bytes as f64;
    let a = am.total_encoded_bytes as f64;
    let ratio = if y > 0.0 { a / y } else { 0.0 };
    eprintln!(
        "encode size  yrs = {} | automerge = {} | ratio (am/yrs) = {:.2}x",
        human_bytes(yrs.total_encoded_bytes),
        human_bytes(am.total_encoded_bytes),
        ratio
    );
    let yr = yrs.peak_rss_bytes as f64;
    let ar = am.peak_rss_bytes as f64;
    let rss_ratio = if yr > 0.0 { ar / yr } else { 0.0 };
    eprintln!(
        "peak RSS     yrs = {} | automerge = {} | ratio (am/yrs) = {:.2}x",
        human_rss(yrs.peak_rss_bytes),
        human_rss(am.peak_rss_bytes),
        rss_ratio
    );

    eprintln!();
    eprintln!("Note: these are aggregate numbers across {} collection YDocs.",
        yrs.collection_count);
    eprintln!("Per-collection breakdown is printed above; the encode-comparison");
    eprintln!("writeup cites the median + p95 of `per_collection_bytes` rather than the sum.");
}