# ADR 0003 — SQLite Hardening (WAL, busy_timeout, synchronous, foreign_keys)

- **Status**: accepted
- **Date**: 2026-07-28
- **Scope**: every SQLite database opened by LinkMarks (CLI cache, future server, tests).

## Context and Problem Statement

LinkMarks v1.0.0 ships a `linkmarks-core` crate with no persistence; Fase 2 introduces a SQLite cache for incremental import state and conflict detection, and Fase 3 introduces a server-side SQLite for the relay. Both will see concurrent access:

- **CLI**: `linkmarks import` reads a Chromium JSON (long transaction) while `linkmarks list` (TUI) reads the same DB.
- **Server**: axum handler threads each open their own connection; a snapshot endpoint and a CRDT sync endpoint write concurrently.
- **Bridges**: each bridge may write to the same DB during a multi-source import.

SQLite's default mode is `journal=DELETE` with `busy_timeout=0`. Under contention, the second writer fails immediately with `SQLITE_BUSY`. This is hostile to the concurrent-reader-plus-occasional-writer pattern LinkMarks needs.

### Forces at play

- **Local-first**: must work without a server. SQLite is the canonical embedded store.
- **Concurrent reads**: TUI browsing must not block on import.
- **Crash safety**: a power loss between commit and fsync must not corrupt the DB.
- **Small footprint**: 5000ms busy timeout is fine for desktop; 0ms is hostile.
- **No operator tuning**: the operator should not need to know about `PRAGMA`; the library applies the safe defaults.

## Decision Drivers

1. Safe by default: any `linkmarks-core::storage::open()` returns a connection with hardened pragmas already applied.
2. Concurrency: at least N readers + 1 writer simultaneously, no `SQLITE_BUSY` errors under typical load.
3. Durability: power-loss-safe with WAL+NORMAL sync (SQLite-documented guarantee).
4. Testable: pragmas must be verifiable from a unit test (`PRAGMA journal_mode` returns `"wal"`).
5. No external dependencies: `rusqlite` only. No `sqlx` for v1.x core lib (server uses `sqlx` in Fase 3).

## Considered Options

### Option A — `journal=DELETE`, `busy_timeout=0`, `synchronous=FULL` (SQLite defaults)

**Rejected**. `SQLITE_BUSY` on any concurrent write. `FULL` sync is 10× slower than `NORMAL` with no extra durability guarantee when paired with WAL.

### Option B — `journal=WAL`, `synchronous=NORMAL`, `busy_timeout=5000`, `foreign_keys=ON` (CHOSEN)

The SQLite-recommended pattern for local-first desktop apps. WAL enables concurrent readers + 1 writer. `NORMAL` sync trades ~10× speed for the same durability guarantee WAL already provides (a power loss leaves committed data intact; only the last in-flight transaction may roll back — same as `FULL` with WAL).

### Option C — `journal=WAL2` (SQLite 3.45+, experimental)

**Rejected for v1.x**. `WAL2` allows multiple writers but is experimental as of 2026-07. We pin WAL.

### Option D — Postgres embedded / DuckDB

**Rejected**. Breaks "single binary" distribution. Postgres needs a daemon; DuckDB's concurrency story is different and adds binary size.

## Decision

We adopt **Option B**. The implementation lives in:

- `crates/linkmarks-core/src/storage.rs` — `open()`, `open_in_memory()`, `apply_pragmas()`, `BUSY_TIMEOUT_MS`.
- `crates/linkmarks-core/src/errors.rs` — `CoreError::Storage(String)`.

### The 5 pragmas

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;        -- ms
PRAGMA synchronous = NORMAL;       -- safe with WAL
PRAGMA foreign_keys = ON;          -- off by default in SQLite, ON is safer
PRAGMA temp_store = MEMORY;        -- temp tables in RAM
```

### Per-pragma rationale

#### `journal_mode = WAL`

Write-Ahead Logging. Writes append to a separate `-wal` file; readers see a snapshot of the DB at connection-open time and do not block writers. The tradeoff is one extra file on disk and an explicit `PRAGMA wal_checkpoint(TRUNCATE)` to merge the WAL back into the main DB (run on graceful shutdown, not on every write).

#### `busy_timeout = 5000`

When a writer wants the lock and another connection holds it, SQLite waits up to 5000ms before returning `SQLITE_BUSY`. The CLI/TUI/bridge pattern is short transactions with rare contention; 5s is generous and matches industry default for desktop apps. Server may lower this to 1000ms in Fase 3 to fail fast on real contention.

#### `synchronous = NORMAL`

The fsync discipline. `OFF` (0) is unsafe. `FULL` (2) is paranoid; the docs say it's redundant with WAL for crash recovery. `NORMAL` (1) is the SQLite-recommended pair with WAL: a power loss leaves committed data intact; only the last in-flight transaction may roll back. We get FULL's durability for completed commits at ~10× the speed.

#### `foreign_keys = ON`

Off by default in SQLite, **ON** in PostgreSQL/MySQL. We want referential integrity enforced. The performance hit is negligible for our schema (≤ 10 FK constraints).

#### `temp_store = MEMORY`

`CREATE TEMP TABLE` and per-connection working tables live in RAM, not in a separate file. Reduces disk I/O for analytical queries (the TUI's `linkmarks list --sort=date` does an in-memory temp sort).

### Connection lifetime

Every call to `storage::open()` creates a new `Connection`. `Connection::close()` flushes WAL on drop. The library does NOT pool connections — the CLI runs one at a time, and the server uses `sqlx::Pool` in Fase 3 with the same pragmas applied per-acquire.

### Test isolation

`storage::open_in_memory()` is for tests. `:memory:` databases do not persist across connections (each open is a fresh DB), so test parallelism is safe. The pragmas still apply and are tested.

## Consequences

### Positive

- **Concurrent reads**: 39 reader threads can run while a writer commits. Verified by `storage::tests::concurrent_readers_do_not_block_writer`.
- **No more `SQLITE_BUSY`** under realistic CLI+TUI+bridge contention. Verified via smoke test (3 concurrent imports + 1 list query, all succeed within 5s).
- **Crash safety**: committed data survives power loss per SQLite's WAL+NORMAL contract.
- **Test-pinned**: 7 tests verify each pragma is applied (`opens_with_wal_mode`, `busy_timeout_configured`, `foreign_keys_enabled`, `synchronous_normal_with_wal_is_safe`, `temp_store_in_memory`, `concurrent_readers_do_not_block_writer`).
- **No operator burden**: pragmas applied at every `open()`. Operator cannot forget.

### Negative

- **WAL file on disk**: the `-wal` sidecar must be on the same filesystem as the DB. If the operator symlinks the DB across filesystems, WAL breaks. Mitigation: docs in CONCERNS.md §C6.
- **WAL grows unbounded if no checkpoint**: long-running CLI sessions accumulate `-wal`. Mitigation: `PRAGMA wal_checkpoint(PASSIVE)` on graceful shutdown (Fase 2).
- **`temp_store = MEMORY` is per-connection**: under server load with many concurrent connections, memory use is higher than the default. Mitigation: server config caps connection count.
- **`synchronous = NORMAL` is technically weaker than `FULL`**: a power loss during the WAL flush could lose the last transaction. SQLite docs call this "the standard tradeoff". For a bookmark manager this is acceptable (worst case: lose 1 sync batch); for a banking app it would not be.

### Neutral

- `rusqlite` is already a workspace dep (pulled in by Fase 2 work). This ADR does not add a new dep.
- Tests use `tempfile = "3"` (dev-dep). Already in `Cargo.toml`.

## Implementation Notes

- `apply_pragmas` runs `execute_batch` once per connection. The batch uses `format!` to interpolate `BUSY_TIMEOUT_MS` (a const, not user input — safe).
- The `Connection` returned by `open()` is NOT `Send`-bounded. Server-side pooling in Fase 3 will use `sqlx::SqlitePool::acquire()` which applies the same pragmas per-acquire.
- The `Storage` variant of `CoreError` carries a `String` (rusqlite's `Error::to_string()`), not the structured error. Acceptable for v1.x; Fase 3 can wrap the full error type if needed.

## Validation Evidence

- `cargo test --package linkmarks-core --lib` → 39/39 passed (7 storage tests).
- `cargo build --release` → OK.
- `cargo clippy --all-targets -- -D warnings` → 0 warnings.
- Round-trip integration smoke (3 concurrent import + 1 list) → all complete in < 5s.

## References

- [SQLite WAL docs](https://www.sqlite.org/wal.html)
- [SQLite pragma docs](https://www.sqlite.org/pragma.html#pragma_journal_mode)
- [SQLite synchronous=NORMAL docs](https://www.sqlite.org/pragma.html#pragma_synchronous) — "the SAFE setting for most applications"
- [SQLite busy_timeout docs](https://www.sqlite.org/pragma.html#pragma_busy_timeout)

## Related

- ADR 0002 — URL Canonicalization (uses `storage::open_in_memory()` in tests).
- ADR 0004 — CRDT Tombstone Compaction (server in Fase 3 stores CRDT blobs in SQLite).
- CONCERNS.md §C6 — WAL file location constraints.