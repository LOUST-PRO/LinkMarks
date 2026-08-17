//! HTTP sync client (yrs over reqwest).
//!
//! Connects to the server, seeds itself with /state, applies N local
//! edits, sends them via /sync, applies the server's response, then
//! re-fetches /state and verifies the final byte-for-byte state equals
//! what the server returns.
//!
//! Reports:
//!   - seed round-trip
//!   - local-edit wall time + per-op p50/p99 latency
//!   - sync round-trip (POST /sync) latency
//!   - convergence: hash(local) == hash(server's /state)
//!
//! Usage: `cargo run --release --bin http_sync_client -- [URL] [N_EDITS]`
//!   URL defaults to http://127.0.0.1:8080
//!   N_EDITS defaults to 500

use std::time::Instant;

use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, Map, ReadTxn, StateVector, Transact, WriteTxn};

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
    let n_edits: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    let http = reqwest::Client::builder()
        .build()
        .expect("reqwest client");
    eprintln!("[client] connecting to {url}, {n_edits} local edits");

    // 1. Health check.
    let h = http
        .get(format!("{url}/healthz"))
        .send()
        .await
        .expect("healthz")
        .text()
        .await
        .expect("healthz body");
    assert_eq!(h, "ok", "server healthz returned {h:?}");

    // 2. Seed: GET /state.
    let seed_start = Instant::now();
    let seed_bytes = http
        .get(format!("{url}/state"))
        .send()
        .await
        .expect("GET /state")
        .bytes()
        .await
        .expect("GET /state bytes");
    let seed_dt = seed_start.elapsed();
    eprintln!(
        "[client] seed: {} B in {:.2} ms",
        seed_bytes.len(),
        seed_dt.as_secs_f64() * 1000.0
    );

    let doc = Doc::new();
    {
        let mut t = doc.transact_mut();
        t.get_or_insert_map("bookmarks");
        t.get_or_insert_map("tags_by_bookmark");
        t.get_or_insert_map("meta");
        t.commit();
    }
    {
        let mut t = doc.transact_mut();
        t.apply_update(yrs::Update::decode_v1(&seed_bytes).expect("decode seed"));
        t.commit();
    }

    // 3. Local edits.
    let mut per_op_latencies = Vec::with_capacity(n_edits);
    for i in 0..n_edits {
        let op_start = Instant::now();
        {
            let mut t = doc.transact_mut();
            let bookmarks = t.get_or_insert_map("bookmarks");
            let id = format!("bm_{i:06}");
            let bm = bookmarks.insert(&mut t, id.as_str(), yrs::types::map::MapPrelim::default());
            bm.insert(&mut t, "original_url", format!("https://example.com/{i}"));
            bm.insert(&mut t, "title", format!("Bookmark {i}"));
            bm.insert(&mut t, "source", "Manual");
            bm.insert(&mut t, "archived", false);
            t.commit();
        }
        per_op_latencies.push(op_start.elapsed());
    }
    per_op_latencies.sort_unstable();
    let local_p50 = per_op_latencies[per_op_latencies.len() / 2];
    let local_p99 = per_op_latencies[(per_op_latencies.len() * 99) / 100];
    let local_total: std::time::Duration = per_op_latencies.iter().sum();
    eprintln!(
        "[client] local edits: {n_edits} ops in {:.2} ms | p50 {:.0} µs | p99 {:.0} µs",
        local_total.as_secs_f64() * 1000.0,
        local_p50.as_secs_f64() * 1_000_000.0,
        local_p99.as_secs_f64() * 1_000_000.0,
    );

    // 4. Build sync request: state_vector + update.
    let (sv, update) = {
        let txn = doc.transact();
        let sv = txn.state_vector();
        let update = txn.encode_state_as_update_v1(&StateVector::default());
        (sv, update)
    };
    let sv_bytes = sv.encode_v1();
    let mut req_body = Vec::with_capacity(8 + sv_bytes.len() + update.len());
    req_body.extend_from_slice(&(sv_bytes.len() as u64).to_le_bytes());
    req_body.extend_from_slice(&sv_bytes);
    req_body.extend_from_slice(&update);

    // 5. POST /sync.
    let sync_start = Instant::now();
    let resp = http
        .post(format!("{url}/sync"))
        .body(req_body)
        .header("content-type", "application/octet-stream")
        .send()
        .await
        .expect("POST /sync");
    let resp_bytes = resp.bytes().await.expect("POST /sync bytes");
    let sync_dt = sync_start.elapsed();
    eprintln!(
        "[client] sync: req {} B → resp {} B in {:.2} ms",
        update.len() + sv_bytes.len() + 8,
        resp_bytes.len(),
        sync_dt.as_secs_f64() * 1000.0
    );

    // 6. Apply server response to client.
    {
        let mut t = doc.transact_mut();
        t.apply_update(yrs::Update::decode_v1(&resp_bytes).expect("decode server response"));
        t.commit();
    }

    // 7. Convergence: GET /state again, compare bytes.
    let final_bytes = http
        .get(format!("{url}/state"))
        .send()
        .await
        .expect("GET /state final")
        .bytes()
        .await
        .expect("GET /state final bytes");

    let local_final = {
        let txn = doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default())
    };

    let local_hash = fnv1a_64(&local_final);
    let server_hash = fnv1a_64(&final_bytes);
    eprintln!(
        "[client] convergence: local={} B (hash {:016x}) | server={} B (hash {:016x})",
        local_final.len(),
        local_hash,
        final_bytes.len(),
        server_hash
    );

    if local_hash == server_hash {
        eprintln!("[client] ✅ CONVERGED — local and server state match");
        std::process::exit(0);
    } else {
        eprintln!("[client] ❌ DIVERGED — see sizes above");
        std::process::exit(1);
    }
}

/// FNV-1a 64-bit — simple, deterministic, no_std-friendly.
fn fnv1a_64(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &byte in b {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
