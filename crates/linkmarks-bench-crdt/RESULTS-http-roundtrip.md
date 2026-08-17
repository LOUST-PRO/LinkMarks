# HTTP sync roundtrip — yrs over axum/reqwest

**Build**: `cargo build --release --bins` (opt-level=3, lto=thin, codegen-units=1)
**Topology**: 1 server process (`http_sync_server`) holding a shared `yrs::Doc`
exposed via axum; 1 client process (`http_sync_client`) doing the standard
seed → local-edit → POST /sync → apply-response → verify pattern.

## Scenario A: client-only writes (server is empty seed)

```
[client] seed:                  2 B in 0.81 ms
[client] local edits:           500 ops in 3.41 ms | p50 4 µs | p99 42 µs
[client] sync roundtrip:        req 72,702 B → resp 2 B in 6.50 ms
[client] convergence:           local=72,686 B | server=72,686 B | hash 43a280731ec15d30
[client] ✅ CONVERGED — local and server state match
```

**Interpretation**: end-to-end works. The empty server has no state beyond the
maps' pre-creation, so the response is just 2 B (the empty state vector
encoding). Client and server converge on the same 72.7 KB encoded state.

Per-op edit latency is dominated by the yrs transaction commit cost: 4 µs median,
42 µs p99. Over 500 ops that's 3.41 ms total — ~146k ops/sec on a single thread,
consistent with the contention-throughput numbers (yrs v0.20 in-process).

The 6.50 ms sync roundtrip is dominated by:
- HTTP framing (reqwest + axum both have ~1 ms baseline)
- TCP loopback
- server-side `apply_update` (decompress + integrate into block store)
- server-side `encode_state_as_update_v1` (full state diff against client's SV)

For 500 inserts the response is 2 B because the server has nothing the client
doesn't (we sent the full update). A real two-way sync would split the diff:
server has some local changes, client has some, both exchange.

## Wire framing (intentionally minimal)

```
POST /sync body:
  [8B u64 LE = sv_len][sv_len B = state vector][rest = yrs update bytes]

200 OK body:
  [yrs update bytes — server's full state relative to client's state vector]
```

In production we'd use the built-in `yrs::sync::SyncMessage` v1/v2 framing
(handles step-1/step-2 protocol correctly). The spike framing is explicit so
we can audit every byte. The trade-off is that we don't get the protocol's
natural ping-pong: a real sync needs two roundtrips.

## Files

- `src/bin/http_sync_server.rs` — axum server, three routes (`/healthz`,
  `/state`, `/sync`), one shared `Arc<Doc>` behind `tokio::sync` semantics.
- `src/bin/http_sync_client.rs` — reqwest client, FNV-1a-64 hash of final
  state, exits 0 on convergence, 1 on divergence.

## Reproduction

```bash
# Terminal 1
PORT=18080 cargo run --release -p linkmarks-bench-crdt --bin http_sync_server

# Terminal 2
cargo run --release -p linkmarks-bench-crdt --bin http_sync_client -- \
  http://127.0.0.1:18080 500
```

Note: port 8080 is occupied by GAVO data center on this laptop, so use 18080
or any free port via `PORT=<free>` env var.

## What's validated

- yrs encoding/decoding roundtrips byte-exact
- State vector exchange + apply_update converges both sides
- axum server can hold a shared `yrs::Doc` across multiple POSTs
- reqwest client can serialize + POST + apply + verify
- Hash match is byte-exact (FNV-1a-64 of full-state encode)

## What's NOT validated (out of scope for spike)

- Two-way concurrent edits (the scenario above is client-writes-only)
- Persistence (in-memory only; server restart = state lost)
- Auth (no API key check; the spike ignores the auth surface)
- LZ4 transport (the spike sends raw yrs bytes; the LZ4 layer goes on top
  of yrs encoding at the gRPC / HTTP middleware level)
- Multi-collection routing (the spike has a single shared Doc; production
  will have one Doc per collection, routed by URL)
- Conflict resolution semantics (the spike uses simple LWW via MapRef
  semantics; production will use `YMap<tag, 1>` element-wise OR + LWW
  remove)

## Combined picture

| metric | yrs v0.20 | automerge v0.5.12 | winner |
|---|---:|---:|---|
| Cold encode size (10k bookmarks) | 4.57 MB | 721.57 KB | automerge (LZ4, 6.4×) |
| Encode delta (4k contended ops) | 1.12 MB | 80.4 KB | automerge (LZ4, 13.9×) |
| Peak RSS (cold) | 60.1 MB | 59.9 MB | tie |
| RSS during sustained writes | 16.5 MB | 37.4 MB | **yrs (2.27× less)** |
| Write throughput (4 threads × 1k) | 82.9 ms | 469.9 ms | **yrs (5.67× faster)** |
| Write p99 latency | 0.4 ms | 7.8 ms | **yrs (5-19× lower)** |
| HTTP roundtrip convergence | ✅ | (not tested) | yrs |

**Verdict**: **yrs at the application layer + LZ4 at the transport
layer** — best of both. yrs gives the throughput/RSS/tail-latency wins;
LZ4 (already present in automerge, also available standalone) gives the
wire-size wins. This is option (C) from the encode-comparison decision
matrix.

Next: integrate the dry-run sync subcommand in `linkmarks-cli` and ship
the self-host template at `docs/relay-deployment.md`.