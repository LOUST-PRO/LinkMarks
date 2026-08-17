//! `compare-concurrent` — contention-throughput driver.
//!
//! 4 threads × 1000 inserts on a single collection YDoc for both
//! yrs and automerge. Prints wall time + per-thread latency +
//! final RSS + encode delta. Run with `--release`.

use linkmarks_bench_crdt::{automerge_concurrent, yrs_concurrent};

fn main() {
    let thread_count = 4;
    let inserts_per_thread = 1_000;

    eprintln!("=== compare-concurrent (contention-throughput suite) ===");
    eprintln!(
        "Threads: {thread_count} | Inserts/thread: {inserts_per_thread} | \
         Total inserts: {}",
        thread_count * inserts_per_thread
    );
    eprintln!();

    eprintln!("--- yrs v0.20 (same YDoc, contended) ---");
    let y = yrs_concurrent::measure(thread_count, inserts_per_thread);
    print_yrs_report(&y);
    eprintln!();

    eprintln!("--- automerge v0.5.12 (same YDoc, Arc<Mutex<Automerge>>) ---");
    let a = match automerge_concurrent::measure(thread_count, inserts_per_thread) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("automerge_concurrent::measure failed: {e}");
            std::process::exit(2);
        }
    };
    print_am_report(&a);
    eprintln!();

    eprintln!("--- comparison ---");
    eprintln!(
        "wall time      yrs = {:>10} µs | automerge = {:>10} µs | yrs is {:.2}x faster",
        y.wall_time_micros,
        a.wall_time_micros,
        a.wall_time_micros as f64 / y.wall_time_micros.max(1) as f64
    );
    eprintln!(
        "encode delta   yrs = {:>10} B  | automerge = {:>10} B  | ratio (am/yrs) = {:.2}x",
        y.encode_delta_bytes,
        a.encode_delta_bytes,
        a.encode_delta_bytes as f64 / y.encode_delta_bytes.max(1) as f64
    );
    eprintln!(
        "final RSS      yrs = {:>10} B  | automerge = {:>10} B  | ratio (am/yrs) = {:.2}x",
        y.final_rss_bytes,
        a.final_rss_bytes,
        a.final_rss_bytes as f64 / y.final_rss_bytes.max(1) as f64
    );
    eprintln!();
    eprintln!("Note: both libraries serialize writes on a single YDoc. The");
    eprintln!("\"throughput ceiling\" measured here is the per-transaction lock");
    eprintln!("cost, not true parallelism. In production each collection is its");
    eprintln!("own sub-doc, so cross-collection writes do NOT contend (no global");
    eprintln!("lock). The numbers below are the worst case for write throughput");
    eprintln!("within a single collection.");
}

fn print_yrs_report(r: &yrs_concurrent::ConcurrencyReport) {
    eprintln!("total inserts: {}", r.total_inserts);
    eprintln!("wall time: {} µs", r.wall_time_micros);
    eprintln!("per-thread p50 µs: {:?}", r.per_thread_p50_micros);
    eprintln!("per-thread p95 µs: {:?}", r.per_thread_p95_micros);
    eprintln!("per-thread p99 µs: {:?}", r.per_thread_p99_micros);
    eprintln!("final RSS: {} B", r.final_rss_bytes);
    eprintln!(
        "encode: {} B → {} B (delta {} B)",
        r.initial_encode_bytes, r.final_encode_bytes, r.encode_delta_bytes
    );
}

fn print_am_report(r: &automerge_concurrent::ConcurrencyReport) {
    eprintln!("total inserts: {}", r.total_inserts);
    eprintln!("wall time: {} µs", r.wall_time_micros);
    eprintln!("per-thread p50 µs: {:?}", r.per_thread_p50_micros);
    eprintln!("per-thread p95 µs: {:?}", r.per_thread_p95_micros);
    eprintln!("per-thread p99 µs: {:?}", r.per_thread_p99_micros);
    eprintln!("final RSS: {} B", r.final_rss_bytes);
    eprintln!(
        "encode: {} B → {} B (delta {} B)",
        r.initial_encode_bytes, r.final_encode_bytes, r.encode_delta_bytes
    );
}