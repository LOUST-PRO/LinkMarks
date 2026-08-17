//! Concurrent inserts on a single collection YDoc (automerge).
//!
//! Mirror of `yrs_concurrent` for the automerge baseline. The
//! concurrency model differs: `automerge::Automerge` is `Send` but not
//! `Sync`, so we wrap it in `Arc<Mutex<_>>` and serialize access.
//! (yrs uses an internal lock in `Doc` that lets `transact_mut()` be
//! called from any thread; the lock is held for the duration of the
//! transaction.)
//!
//! Same measurement outputs: wall time, per-thread latency p50/p95/p99,
//! final RSS, encode delta.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use automerge::transaction::Transactable;
use automerge::{Automerge, AutomergeError, ObjType, ROOT};

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

pub fn measure(thread_count: usize, inserts_per_thread: usize) -> Result<ConcurrencyReport, AutomergeError> {
    let doc = Arc::new(Mutex::new(Automerge::new()));

    // Pre-create the maps (avoid measuring the first-put cost on every
    // transaction).
    {
        let mut d = doc.lock().expect("automerge mutex poisoned");
        let mut tx = d.transaction();
        tx.put_object(ROOT, "bookmarks", ObjType::Map)?;
        tx.put_object(ROOT, "tags_by_bookmark", ObjType::Map)?;
        tx.commit();
    }

    let initial_encode = {
        let d = doc.lock().expect("automerge mutex poisoned");
        d.save().len()
    };

    let start = Instant::now();
    let mut handles = Vec::with_capacity(thread_count);
    for tid in 0..thread_count {
        let doc = doc.clone();
        handles.push(thread::spawn(move || -> Result<Vec<u128>, AutomergeError> {
            let mut latencies = Vec::with_capacity(inserts_per_thread);
            for i in 0..inserts_per_thread {
                let bookmark_id = format!("t{tid:02}_b{i:05}");
                let op_start = Instant::now();
                {
                    let mut d = doc.lock().expect("automerge mutex poisoned");
                    let mut tx = d.transaction();
                    let bookmarks_map = tx.put_object(ROOT, "bookmarks", ObjType::Map)?;
                    let tags_map = tx.put_object(ROOT, "tags_by_bookmark", ObjType::Map)?;
                    let bm = tx.put_object(&bookmarks_map, &bookmark_id, ObjType::Map)?;
                    tx.put(&bm, "original_url", format!("https://contended.example/{tid}/{i}"))?;
                    tx.put(&bm, "canonical_url", format!("https://contended.example/{tid}/{i}"))?;
                    tx.put(&bm, "title", format!("Contended bookmark {tid}-{i}"))?;
                    tx.put(&bm, "source", "Manual")?;
                    tx.put(&bm, "archived", false)?;
                    let tags = tx.put_object(&tags_map, &bookmark_id, ObjType::Map)?;
                    tx.put(&tags, "contended", 1i64)?;
                    tx.commit();
                }
                latencies.push(op_start.elapsed().as_micros());
            }
            Ok(latencies)
        }));
    }

    let mut per_thread_lats: Vec<Vec<u128>> = Vec::with_capacity(thread_count);
    for h in handles {
        let lats = h.join().expect("thread panicked")?;
        per_thread_lats.push(lats);
    }

    let wall_time = start.elapsed();

    let final_encode = {
        let d = doc.lock().expect("automerge mutex poisoned");
        d.save().len()
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

    Ok(ConcurrencyReport {
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
    })
}

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