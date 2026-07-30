# ADR 0004 — CRDT Tombstone Compaction Protocol (Fase 3 Prep)

- **Status**: proposed (Fase 3 prep)
- **Date**: 2026-07-28
- **Scope**: LinkMarks server (Fase 3) CRDT state management; `yrs`-based sync.
- **Supersedes**: nothing.

## Context and Problem Statement

LinkMarks Fase 3 introduces multi-device sync via a CRDT (provisionally `yrs` / Y-CRDT, pending the Fase 3 spike vs `automerge-rs`). The CRDT represents the bookmark collection as a shared replicated state, and deletions are encoded as **tombstones** — special markers that propagate to all replicas to ensure convergent semantics.

The problem: tombstones accumulate. Every deletion of a bookmark, tag, or collection adds a tombstone that ALL replicas must acknowledge to safely drop. In an active user with 10 years of activity, the tombstone log can grow unboundedly. The server holds a full history; the client merges on every reconnect.

### Why this matters for LinkMarks

- **Local-first promise**: clients work offline. If a client is offline during a compaction, it diverges from the server's compacted state.
- **Multi-device**: phone + laptop + tablet each hold their own replicas. Compaction must be safe across N replicas with N different offline durations.
- **Operator trust**: a user who sees "your bookmark disappeared" because of bad compaction will leave.
- **Storage cost**: the server is a relay; it should NOT have to store every bookmark's deletion history forever. Storage is cheap but not free, and tombstone growth is superlinear with user churn.

### Forces at play

- **CRDT invariants**: we cannot drop tombstones that ANY replica still needs. The CRDT contract is "merging must be commutative, associative, idempotent".
- **No coordinator**: CRDTs are decentralized. Compaction is a server-only optimization; clients don't see tombstones.
- **Forward progress**: compaction should NOT block live writes. A user importing 1000 bookmarks should not wait for GC.
- **Recovery safety**: if compaction goes wrong, we must be able to roll back.

## Decision Drivers

1. **Bounded storage**: server-side tombstone count must stabilize, not grow forever.
2. **Client safety**: any client that was synced within the last N days must not diverge.
3. **No client protocol change**: compaction is invisible to clients that are up-to-date.
4. **Audit trail**: every compaction emits a structured log entry for operator review.
5. **Reversible**: a bad compaction can be undone by keeping the pre-compaction snapshot for 30 days.

## Considered Options

### Option A — never compact (keep all tombstones forever)

**Rejected**. Server storage grows unboundedly. After 10 years, a 100-bookmark/year user has 1000 tombstones. After 100 devices, 100000 tombstones. Not acceptable.

### Option B — compact based on timestamp (e.g. drop tombstones older than 90 days)

**Rejected**. A client that was offline for 91 days loses data. Violates the local-first promise. Unacceptable for a bookmark manager.

### Option C — compact based on per-client vector-clock ack (CHOSEN)

The server tracks per-client `(device_id, last_acked_tombstone_id)` and refuses to compact a tombstone until ALL known devices have acked it. Devices that never reconnect are eventually marked "abandoned" after a long grace period (180 days), at which point their tombstones are released. Devices that reconnect after being marked abandoned perform a full resync (download the compacted state from scratch).

### Option D — HLC (Hybrid Logical Clock) per tombstone + global watermark

A simpler variant of C: every tombstone has an HLC timestamp; compaction picks a watermark and drops all tombstones before it. Clients that reconnect after the watermark perform a partial merge. **Rejected** because it requires clients to track their own watermark, which is fragile.

## Decision

We adopt **Option C** (per-client ack). The implementation outline:

### Server-side protocol

```text
1. Server records `(tombstone_id, op_hash, device_ids_pending_ack)` for each
   deletion operation as it arrives from any device.

2. Server endpoint `POST /v1/tombstones/ack` accepts
   `{device_id, tombstone_ids: [string]}` and marks those tombstones
   as acked by that device.

3. Compaction job runs nightly:
   - For each tombstone, check if `device_ids_pending_ack` is empty.
   - If empty, mark tombstone as `compacted`.
   - Group contiguous compacted tombstones into a snapshot.
   - Replace the document history with the snapshot + new operations.

4. Pre-compaction snapshots are kept for 30 days under
   `~/.local/share/linkmarks-relay/snapshots/<timestamp>.bin`
   for rollback.

5. Devices that reconnect with an unknown tombstone ID (e.g. they were
   offline during compaction) trigger a full resync:
   `GET /v1/snapshot?device_id=X&since=<their_last_known_version>`
   returns the current compacted state. The client merges.
```

### Client-side protocol

```text
On every successful sync:
  client.ack_tombstones(server_tombstone_ids_seen_this_sync)

On reconnect with unknown tombstone:
  client.full_resync(server.snapshot())
```

The ack is best-effort: if the client crashes before acking, the next sync re-acks. Idempotent.

### Configurable grace period

`config.toml`:

```toml
[relay.compaction]
grace_period_days = 180
snapshot_retention_days = 30
min_acked_devices_to_compact = 0.9   # 90% of known devices
```

The `min_acked_devices_to_compact` knob handles the "one device lost forever" case: if 90% of devices have acked, the server compacts and marks the remaining 10% as needing full resync.

### Storage format

Tombstones are stored in a SQLite table (Fase 3 server):

```sql
CREATE TABLE crdt_tombstones (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    op_hash BLOB NOT NULL,
    created_at INTEGER NOT NULL,    -- unix seconds
    compacted_at INTEGER,           -- null if live
    snapshot_id TEXT                -- null if live
);
CREATE INDEX crdt_tombstones_live_idx ON crdt_tombstones(document_id) WHERE compacted_at IS NULL;
CREATE TABLE crdt_tombstone_acks (
    tombstone_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    acked_at INTEGER NOT NULL,
    PRIMARY KEY (tombstone_id, device_id)
);
```

WAL mode (ADR 0003) applies.

### Audit log

Each compaction emits a JSONL line:

```json
{"ts": 1753700000, "event": "compaction.completed", "document_id": "...", "tombstones_compacted": 1234, "remaining_live": 12, "snapshot_id": "snap-2026-07-28T00:00:00Z"}
```

Stored at `~/.local/share/linkmarks-relay/audit/compaction.jsonl`.

## Consequences

### Positive

- **Bounded storage**: tombstone count stabilizes once devices ack. A user with 5 active devices converges within days; long-tail devices converge when they next sync.
- **Local-first preserved**: devices that stay online are never affected. Devices that go offline for ≤ grace_period do not lose data.
- **Forward progress**: compaction is async (nightly job), no live-write blocking.
- **Reversible**: 30-day snapshot retention allows rollback if a bug surfaces.
- **Auditable**: every compaction event is in JSONL.

### Negative

- **Abandoned devices lose offline changes** after grace_period + snapshot_retention. The user gets a fresh server snapshot, NOT their old offline state. Mitigation: UI shows "this device was offline for X days; we restored from server backup".
- **Compaction is server-only**: a CLI-only user (no relay) keeps tombstones forever locally. Mitigation: a `--gc` flag in the CLI does the same compaction client-side, bounded by `last_sync_age`.
- **Network round-trips on reconnect**: an offline device does N acks + 1 full snapshot fetch. Cost: ~1MB per device per reconnect. Acceptable.
- **Server complexity**: new endpoint (`POST /v1/tombstones/ack`), new cron job, new audit table. Estimated ~600 LOC in the relay crate.

### Neutral

- The protocol is forward-compatible with `automerge-rs` if the Fase 3 spike rejects `yrs` — both libraries expose tomb IDs and ack patterns.
- The server is the ONLY component that needs to implement this. Clients just ack.
- Compaction is invisible to v1.x users; the protocol is Fase 3 work.

## Implementation Notes

- The server is `linkmarks-server` (Fase 3, axum + sqlx + yrs).
- The compaction job is a `tokio::spawn` background task scheduled via `tokio::time::interval`.
- The snapshot format is `yrs::encode_state_as_update_v1(&doc)` — the binary blob is the document's full state.
- The pre-compaction snapshot is `yrs::encode_state_as_update_v1(&doc.pre_compaction_state)`.

## Validation Evidence (Fase 3, when implemented)

- Spike test: 100 simulated devices, 10k tombstone ops, 30-day sim → compaction converges, no client data loss.
- Chaos test: kill server mid-compaction → recovery via pre-compaction snapshot.
- Load test: nightly compaction on 100GB history → completes in < 10 minutes.

## Related

- ADR 0003 — SQLite Hardening (storage layer for the server).
- ARCHITECTURE.md §Fase 3 — CRDT sync architecture.
- CONCERNS.md §C2 — CRDT library choice (`yrs` vs `automerge-rs`).
- [Y-CRDT docs](https://docs.y-crdt.dev/) — tombstone semantics.