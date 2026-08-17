# Encode comparison — 10k-bookmark workload

**Workload**: 10,000 synthetic bookmarks (seed=42, deterministic)
**Library versions**: yrs 0.20.0, automerge 0.5.12
**Pattern**: per-collection sub-doc, `YMap<tag, 1>` per bookmark
**Build**: release + `lto = "thin"` + `codegen-units = 1` + `opt-level = 3`
**Runner**: `cargo run --release -p linkmarks-bench-crdt --bin compare`

## Headline numbers

| Dimension | yrs 0.20 | automerge 0.5.12 | Ratio (am/yrs) |
|---|---|---|---|
| **Total encode (sum of 15 collections)** | **4.57 MB** | **721.57 KB** | **0.15×** (automerge 6.4× smaller) |
| **Peak RSS** (all 15 YDocs live) | 60.11 MB | 59.93 MB | 1.00× (tied) |
| Collection count | 15 | 15 | — |
| Bookmark count | 10,000 | 10,000 | — |

**Per-collection encode size**:

| Collection | yrs | automerge | Ratio (am/yrs) |
|---|---|---|---|
| `inbox` (≈700 uncollected bookmarks) | 1.58 MB | 264.38 KB | 0.16× |
| `tools` | 237.49 KB | 35.19 KB | 0.14× |
| `work` | 227.93 KB | 33.68 KB | 0.14× |
| `personal` | 227.54 KB | 33.74 KB | 0.14× |
| `ai` | 224.21 KB | 33.30 KB | 0.14× |
| `rust` | 221.42 KB | 32.94 KB | 0.14× |
| `archive` | 220.67 KB | 32.79 KB | 0.14× |
| `security` | 219.32 KB | 32.59 KB | 0.14× |
| `to-read` | 217.94 KB | 32.41 KB | 0.14× |
| `research` | 218.14 KB | 32.41 KB | 0.14× |
| `reference` | 215.62 KB | 32.16 KB | 0.14× |
| `shopping` | 212.00 KB | 31.64 KB | 0.14× |
| `ops` | 209.77 KB | 32.03 KB | 0.15× |
| `learning` | 208.08 KB | 30.98 KB | 0.14× |
| `design` | 205.86 KB | 31.34 KB | 0.15× |

## Findings

1. **encode size: automerge is 6.4× smaller.** The encode-size
   assumption that originally pointed at yrs does **not** hold up.
   Two factors likely contribute:
   - **LZ4 compression** (automerge 0.5+ applies it on `save()`).
     To confirm, the next measurement will disable it and re-measure
     both libraries raw. If automerge raw matches yrs, the gap is
     purely compression.
   - **Op metadata overhead**: yrs uses varint-per-key with client
     ID + clock per op; automerge uses a similar scheme but with
     delta-aware sharing that yrs 0.20 may not exploit as aggressively.
2. **Peak RSS is tied at ~60 MB.** The "yrs wins on server RSS budget"
   assumption does NOT hold up. Both libraries hold the full op log
   in memory; the encode-size difference is mostly offset by RSS
   being dominated by op structures, not bytes.
3. **`inbox` is uniformly larger** in both libraries (1.58 MB / 264 KB)
   because the fixture's ~700 uncollected bookmarks land there. Same
   shape, different scale. The proportional ratio (am/yrs ≈ 0.14-0.16)
   is consistent across all collections, which is a good sign for
   measurement validity.
4. **Per-bookmark overhead** (approx): yrs ≈ 480 B/bookmark, automerge ≈
   74 B/bookmark. At 10k bookmarks: 4.8 MB vs 740 KB.

## What this means for the choice

The original signal pointing at yrs was based on:
- server RSS budget — **disproven by measurement (tied)**
- encode size — **disproven by measurement (yrs is 6.4× larger)**
- maturity — yrs is more mature (Y-CRDT spec lineage); still favors yrs
- bindings — yrs has Rust-native; still favors yrs

The choice needs to be revisited. Candidates:
- **A. Switch to automerge.** Smaller bytes, equal RSS, but: (a)
  less mature, (b) Rust binding is itself less battle-tested,
  (c) the LZ4-compression advantage is "free" but ties us to
  automerge's wire format.
- **B. Stay with yrs.** Reasons: maturity, Rust-native, no
  transport-format lock-in (yrs is the Y-CRDT reference impl).
  Cost: 6× larger over-the-wire bytes, which matters for self-hosted
  VPS users on metered links.
- **C. Layer LZ4 over yrs.** Encode with yrs, compress the
  payload at the transport layer (flate2/zstd). Pays the cost in
  the transport code, not the library choice. Preserves yrs
  maturity + gives automerge-competitive bytes.

The contention-throughput and HTTP-roundtrip suites give more signal
— especially around how each library behaves under contention and
how the wire format affects sync latency. See
[`RESULTS-contention-throughput.md`](./RESULTS-contention-throughput.md)
and [`RESULTS-http-roundtrip.md`](./RESULTS-http-roundtrip.md) for the
follow-up measurements.

## Reproducibility

The fixture is deterministic (seed=42, `ChaCha8Rng`). Re-run:

```bash
cd <repo-root>
cargo run --release -p linkmarks-bench-crdt --bin compare
```

Expected output: identical numbers modulo OS noise on RSS (RSS
is process-level; encode-size is deterministic).

## Open questions for the contention suite

- Under contention (4 threads × 1000 inserts), does the RSS ratio
  hold? (Hypothesis: automerge's `im::HashMap` (persistent) has
  worse write-amp than yrs's BTree-based op log.)
- Does p50 insert latency favor yrs or automerge at the 1k-10k
  bookmark scale?
- Convergence check: 2 client YDocs + sync via state vector +
  apply update — does either fail to converge?

## Source of measurement code

- `crates/linkmarks-bench-crdt/src/yrs_measure.rs` — yrs encode + RSS
- `crates/linkmarks-bench-crdt/src/automerge_measure.rs` — automerge encode + RSS
- `crates/linkmarks-bench-crdt/src/bin/compare.rs` — driver

All compiled clean under `--release` with LTO.