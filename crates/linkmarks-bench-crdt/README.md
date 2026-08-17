# `linkmarks-bench-crdt`

Benchmark suite that backs the CRDT choice in the project README.

Measures two Rust CRDT candidates — `yrs` v0.20 and `automerge` v0.5 — across the three workloads that matter for LinkMarks multi-device sync:

| Suite | Binary | Measures |
|---|---|---|
| Encode comparison | `compare` | Cold encode size + peak RSS, 15 collections × ~400 bookmarks |
| Contention throughput | `compare-concurrent` | Write throughput, per-thread p50/p95/p99, RSS during sustained writes |
| HTTP roundtrip | `http_sync_server` + `http_sync_client` | End-to-end axum/reqwest sync with byte-exact convergence check |

The full per-suite writeups live alongside this file as `RESULTS-*.md`.

## Why a separate workspace member

The candidate CRDTs (`yrs`, `automerge`) and the HTTP roundtrip harness (`axum`, `reqwest`, `tokio`) pull in ~10 MB of compiled deps. Compiling them only on demand — `cargo run -p linkmarks-bench-crdt --bin compare` etc. — keeps them out of the path that builds `linkmarks-cli` for distribution. The `publish = false` flag in `Cargo.toml` keeps them out of crates.io.

## Build

```bash
cargo build --release -p linkmarks-bench-crdt --bins
```

## Run

```bash
# Encode comparison
cargo run --release -p linkmarks-bench-crdt --bin compare

# Contention throughput
cargo run --release -p linkmarks-bench-crdt --bin compare-concurrent

# HTTP roundtrip (two terminals)
PORT=18080 cargo run --release -p linkmarks-bench-crdt --bin http_sync_server
cargo run --release -p linkmarks-bench-crdt --bin http_sync_client -- http://127.0.0.1:18080 500
```

## Layout

```text
crates/linkmarks-bench-crdt/
├── Cargo.toml                        # package + 6 binaries
├── README.md                         # this file
├── src/
│   ├── lib.rs                        # re-exports + sanity tests
│   ├── fixture.rs                    # deterministic 10k-bookmark generator (seed=42)
│   ├── yrs_measure.rs                # yrs encode + RSS measurement
│   ├── automerge_measure.rs          # automerge encode + RSS measurement
│   ├── yrs_concurrent.rs             # yrs contention throughput
│   ├── automerge_concurrent.rs       # automerge contention throughput
│   └── bin/
│       ├── yrs-bench.rs              # encode/decode scaffold
│       ├── automerge-bench.rs        # encode/decode scaffold
│       ├── compare.rs                # encode-comparison driver
│       ├── compare-concurrent.rs     # contention-throughput driver
│       ├── http_sync_server.rs       # axum server, three routes
│       └── http_sync_client.rs       # reqwest client, FNV-1a-64 verify
└── RESULTS-*.md                      # measurement writeups (one per suite)
```

## Decision criteria

Switch from `yrs` to `automerge-rs` only if automerge wins **≥3 of 6
dimensions by ≥2×**:

1. Encode size (1k / 10k / 100k bookmarks)
2. Op latency (p50 / p95 / p99, in-memory)
3. Memory RSS (after 10k-bookmark doc load)
4. Sync latency (two-client server relay)
5. Convergence under concurrent inserts (zero conflicts required)
6. Persistence durability (kill server mid-sync, verify recovery)

The actual decision (and the measurement numbers) live in
[`RESULTS-encode-comparison.md`](./RESULTS-encode-comparison.md),
[`RESULTS-contention-throughput.md`](./RESULTS-contention-throughput.md),
and [`RESULTS-http-roundtrip.md`](./RESULTS-http-roundtrip.md).
The result: **yrs at the application layer + LZ4 at the transport layer.**

## Fixture provenance

Synthetic — does not mirror any real user's bookmarks. The struct shape mirrors `linkmarks-core::Bookmark` (id, original_url, canonical_url, title, description, tags, collection, created_at, updated_at, source, content_type, archived) but `linkmarks-core` is **not imported** as a path dep: the bench stays compilable independently of `linkmarks-core`'s release cycle.

Deterministic via `rand::SeedableRng::seed_from_u64(42)` → `rand_chacha::ChaCha8Rng`. Re-running the generator byte-exactly recreates the same workload.

## What this is NOT

- **Not** part of the production binary. The suite exists to back the
  performance numbers cited in the project README; production code lives
  in `linkmarks-core`, `linkmarks-server`, and `linkmarks-cli`.
- **Not** wired into `debian/` / `rpm/` / `arch/` (the bench never reaches
  end users).
- **Not** in the main CI workflow (`.github/workflows/ci-smoke.yml`) by
  default — opt-in via `-p linkmarks-bench-crdt`.
- **Not** a security review. API-key auth lands with the relay binary.
