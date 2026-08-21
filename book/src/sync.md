# Sync model

This page documents the multi-device sync model used by LinkMarks
v2.2.0 and later. The sync layer is **preview** in v2.2.0 — the
CLI is fully wired and tested, but the relay binary is in a
future release. The data model and conflict-resolution rules are
stable.

## Design goals

1. **Local-first.** A device that has never run `linkmarks sync`
   is functionally identical to one that syncs daily. Sync is
   additive, never required.
2. **Operator-controlled.** The relay is self-hosted; the
   operator controls the server, the storage, the retention, and
   the access logs.
3. **Privacy by design.** The relay sees opaque yrs bytes, not
   plaintext bookmarks. The relay cannot decrypt without the
   device-shared key, which is configured out-of-band.
4. **Conflict-free.** Two devices editing the same bookmark
   concurrently converge to the same final state without
   operator intervention.
5. **Auditable.** The sync layer is ~600 LOC and lives in
   `linkmarks-core/src/sync/`; every merge operation has unit
   tests with deterministic fixtures.

## Architecture

```text
   ┌──────────┐                  ┌──────────┐                  ┌──────────┐
   │ Device A │                  │  Relay   │                  │ Device B │
   │  SQLite  │                  │  (yrs    │                  │  SQLite  │
   │  + yrs   │  ──yrs bytes──▶  │  opaque  │ ──yrs bytes──▶  │  + yrs   │
   │  snapshot│                  │  storage)│                  │  snapshot│
   └──────────┘                  └──────────┘                  └──────────┘
```

Each device has a local SQLite store. Sync serialises the
changed rows into a per-collection yrs sub-document and pushes
the opaque bytes to the relay. The relay stores the bytes keyed
by a per-collection name (`bookmarks`, `tags`, `folders`) and a
per-device `device_id`.

On pull, the device fetches the latest remote bytes for each
collection and merges them into its local yrs sub-document. The
SQLite rows are then re-derivable from the merged yrs snapshot.

## The relay

The relay (`linkmarks-relay`, future release) is a tiny HTTP
server with 4 endpoints:

```text
POST /v1/push/{collection}
     Headers: Authorization: Bearer <device-token>
     Body: opaque yrs bytes
     Response: 204 No Content

GET  /v1/pull/{collection}
     Headers: Authorization: Bearer <device-token>
     Response: 200 OK with body = opaque yrs bytes

GET  /v1/state/{collection}
     Headers: Authorization: Bearer <device-token>
     Response: { "version": <u64>, "device_count": <u32> }

GET  /healthz
     Response: 200 OK with body = { "version": "<relay-version>" }
```

The relay stores bytes in a per-collection file under
`/var/lib/linkmarks-relay/`. There is no index, no query layer,
no plaintext ever.

## Conflict resolution

Conflicts arise when two devices edit the same bookmark
concurrently. The yrs CRDT resolves them by Lamport timestamp
order. Specifically:

1. Each device tracks a monotonically-increasing `lamport_clock`
   counter.
2. Every mutation is stamped with `(device_id, lamport_clock)`.
3. On merge, the mutation with the higher `lamport_clock` wins.
4. Ties are broken by `device_id` lexicographic order.

Bookmark fields are merged per-field (not per-record), so two
devices editing different fields of the same bookmark never
overwrite each other.

Example:

```text
Device A (clock=42): edit https://example.com -> title="Example"
Device B (clock=41): edit https://example.com -> tags=["rust"]
```

After merge, the bookmark has `title="Example"` (from A, clock 42)
and `tags=["rust"]` (from B, clock 41). The merge is automatic;
neither edit is lost.

## What syncs

The sync layer covers 4 entity types:

| Entity | Per-collection name | Conflict policy |
|---|---|---|
| `bookmarks` | `bookmarks` | field-level merge |
| `tags` | `tags` | set union |
| `folders` | `folders` | last-writer-wins (tree shape is single-writer in practice) |
| `sync_log` | not synced | per-device, never shared |

The `bookmark_tags` and `bookmark_folders` join tables are
derived locally from `bookmarks` + `tags` + `folders` after each
pull.

## What does NOT sync

- The local `config.toml` — each device has its own
  configuration.
- The local `keymap.toml` — same.
- The `last_visit_at` and `visit_count` fields — these are
  per-device browsing activity, not shared bookmarks.
- The original URL — only the canonical URL is synced. The
  first device to see a URL wins for `original_url`.

## Sync CLI

```bash
# Push local changes to the relay
linkmarks sync push

# Pull remote changes into local
linkmarks sync pull

# Show sync state
linkmarks sync status

# Push with custom relay URL
linkmarks sync push --remote https://relay.example.com
```

`linkmarks sync` is idempotent — pushing with no local changes
is a no-op; pulling with no remote changes is a no-op.

## Sync timing

By default, `linkmarks sync` runs when invoked. There is no
built-in cron. Operators who want background sync typically
wrap it in a systemd timer:

```ini
# ~/.config/systemd/user/linkmarks-sync.service
[Unit]
Description=LinkMarks background sync
After=network-online.target

[Service]
Type=oneshot
ExecStart=/usr/bin/linkmarks sync push
ExecStart=/usr/bin/linkmarks sync pull

# ~/.config/systemd/user/linkmarks-sync.timer
[Unit]
Description=Run LinkMarks sync every 15 minutes

[Timer]
OnCalendar=*:0/15
Persistent=true

[Install]
WantedBy=timers.target
```

## Threat model

The relay's threat model is documented in the
[Hardening](./hardening.md#relay-threat-model) page. The short
version: the relay is untrusted from the perspective of the
bookmark contents; it stores opaque bytes and cannot decrypt.
The relay is trusted from the perspective of availability
and storage integrity.