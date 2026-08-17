//! Concurrent inserts on a single collection YDoc.
//!
//! Goal: measure write throughput + RSS under contention. Both yrs
//! and automerge serialize writes on a single document (yrs via an
//! internal Mutex in `Doc`, automerge via the type being `!Sync`).
//! So this measurement answers "what's the throughput ceiling when N
//! threads all write to the same collection?" rather than "true
//! parallelism."
//!
//! Pattern (locked decisions #1 + #2):
//! - Single collection YDoc (we pick the largest from the fixture)
//! - N threads, each inserts M synthetic bookmarks
//! - We measure: total wall time, per-thread latency p50/p95/p99,
//!   final RSS, final encode size delta vs initial.

use std::sync::Arc;
use std::thread;
use std::time::Instant;

use yrs::types::map::MapPrelim;
use yrs::{Doc, Map, ReadTxn, StateVector, Transact, WriteTxn};

use crate::fixture::{generate_fixture, BenchBookmark};

#[derive(Debug, Clone)]
pub struct ConcurrencyReport {
    pub thread_count: usize,
    pub inserts_per_thread: usize,
    pub total_inserts: usize,
    pub wall_time_micros: u128,
    pub per_thread_p50_micros: Vec<u128>,
    pub per_thread_p95_micros: Vec<u128>,
    pub per_thread_p99_micros: Vec<u128>,
    pub final_rss_bytes: u64,
    pub initial_encode_bytes: usize,
    pub final_encode_bytes: usize,
    pub encode_delta_bytes: usize,
}

/// Run the concurrent-insert measurement on a fresh YDoc.
///
/// `bookmarks` is the fixture pool; each thread draws `inserts_per_thread`
/// fresh items (deterministic via a per-thread `ChaCha8Rng` would be
/// ideal, but for the contention-throughput suite we use synthetic
/// inserts to keep the measurement apples-to-apples).
pub fn measure(thread_count: usize, inserts_per_thread: usize) -> ConcurrencyReport {
    let doc = Arc::new(Doc::new());

    // Pre-create the bookmarks map (avoids measuring the cost of the
    // first `get_or_insert_map` call on every transaction).
    {
        let mut t = doc.transact_mut();
        t.get_or_insert_map("bookmarks");
        t.get_or_insert_map("tags_by_bookmark");
        t.commit();
    }

    let initial_encode: usize = {
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default()).len()
    };

    let start = Instant::now();
    let mut handles = Vec::with_capacity(thread_count);
    for tid in 0..thread_count {
        let doc = doc.clone();
        handles.push(thread::spawn(move || {
            let mut latencies = Vec::with_capacity(inserts_per_thread);
            for i in 0..inserts_per_thread {
                let bookmark_id = format!("t{tid:02}_b{i:05}");
                let op_start = Instant::now();
                {
                    let mut t = doc.transact_mut();
                    let bookmarks_map = t.get_or_insert_map("bookmarks");
                    let tags_map = t.get_or_insert_map("tags_by_bookmark");
                    let bm = bookmarks_map.insert(&mut t, bookmark_id.as_str(), MapPrelim::default());
                    bm.insert(&mut t, "original_url", format!("https://contended.example/{tid}/{i}"));
                    bm.insert(&mut t, "canonical_url", format!("https://contended.example/{tid}/{i}"));
                    bm.insert(&mut t, "title", format!("Contended bookmark {tid}-{i}"));
                    bm.insert(&mut t, "source", "Manual");
                    bm.insert(&mut t, "archived", false);
                    let tags = tags_map.insert(&mut t, bookmark_id.as_str(), MapPrelim::default());
                    tags.insert(&mut t, "contended", 1i64);
                    t.commit();
                }
                latencies.push(op_start.elapsed().as_micros());
            }
            latencies
        }));
    }

    let mut per_thread_lats: Vec<Vec<u128>> = Vec::with_capacity(thread_count);
    for h in handles {
        per_thread_lats.push(h.join().expect("thread panicked"));
    }

    let wall_time = start.elapsed();

    let final_encode: usize = {
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default()).len()
    };
    let peak_rss = read_rss_bytes();
    std::hint::black_box(&doc);

    let mut p50 = Vec::with_capacity(thread_count);
    let mut p95 = Vec::with_capacity(thread_count);
    let mut p99 = Vec::with_capacity(thread_count);
    for mut lats in per_thread_lats {
        lats.sort_unstable();
        p50.push(percentile(&lats, 50));
        p95.push(percentile(&lats, 95));
        p99.push(percentile(&lats, 99));
    }

    ConcurrencyReport {
        thread_count,
        inserts_per_thread,
        total_inserts: thread_count * inserts_per_thread,
        wall_time_micros: wall_time.as_micros(),
        per_thread_p50_micros: p50,
        per_thread_p95_micros: p95,
        per_thread_p99_micros: p99,
        final_rss_bytes: peak_rss,
        initial_encode_bytes: initial_encode,
        final_encode_bytes: final_encode,
        encode_delta_bytes: final_encode.saturating_sub(initial_encode),
    }
}

/// Linear-interpolated percentile over a sorted slice.
fn percentile(sorted: &[u128], p: u32) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p as f64 / 100.0) * (sorted.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = rank - lo as f64;
        let a = sorted[lo] as f64;
        let b = sorted[hi] as f64;
        (a + (b - a) * frac) as u128
    }
}

/// Stand-in so this file compiles without `cfg(test)` plumbing for the
/// fixture import. (The contention-throughput suite doesn't use the
/// bookmark fixture directly; each thread generates its own
/// synthetic URL/title.)
#[allow(dead_code)]
fn _unused() {
    let _ = generate_fixture(1);
    let _b: Option<&BenchBookmark> = None;
}

fn read_rss_bytes() -> u64 {
    let s = match std::fs::read_to_string("/proc/self/statm") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return 0;
    }
    let pages: u64 = match parts[1].parse() {
        Ok(n) => n,
        Err(_) => return 0,
    };
    pages * 4096
}