# LinkMarks — Architecture

**Status**: Documentation staging only. No repo, no code, no git.
**Date**: 2026-07-28
**Target**: `LOUST-PRO/LinkMarks` (private, scaffolding pending)

## Workspace tree (proposed)

```
linkmarks/                              # LOUST-PRO/LinkMarks (private)
├── Cargo.toml                          # workspace manifest
├── Cargo.lock
├── README.md
├── LICENSE                             # AGPLv3
├── LICENSE-COMMERCIAL.md               # commercial terms
├── CHANGELOG.md
├── crates/
│   ├── linkmarks-core/                 # lib — model + traits + parser
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── model.rs                # Bookmark, Collection, Tag, Source
│   │   │   ├── traits.rs               # BookmarkSource, BookmarkSink
│   │   │   ├── canonical.rs            # URL canonicalization
│   │   │   ├── dedupe.rs               # local deterministic dedupe
│   │   │   └── parser/                 # shared parsing helpers
│   │   └── tests/
│   ├── linkmarks-cli/                  # bin — clap commands
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── cmd/
│   │   │   │   ├── list.rs             # `linkmarks list`
│   │   │   │   ├── import.rs           # `linkmarks import`
│   │   │   │   ├── export.rs           # `linkmarks export`
│   │   │   │   └── dedupe.rs           # `linkmarks dedupe`
│   │   │   └── ui.rs                   # output formatting (table/json/yaml)
│   │   └── tests/
│   ├── linkmarks-tui/                  # bin — ratatui (Fase 2)
│   │   └── ...
│   ├── linkmarks-gui/                  # bin — Dioxus desktop (Fase 4)
│   │   └── ...
│   ├── linkmarks-server/               # bin — axum + sqlx + yrs relay (Fase 3)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── routes.rs
│   │   │   ├── relay.rs                # CRDT relay endpoints
│   │   │   └── store.rs                # sqlx queries
│   │   └── migrations/                 # sqlx-cli compatible
│   └── bridges/
│       ├── linkmarks-bridge-chromium/  # bin — Chromium JSON Bookmarks
│       ├── linkmarks-bridge-firefox/   # bin — Firefox places.sqlite + jsonlz4
│       ├── linkmarks-bridge-netscape/  # lib+bin — HTML import/export
│       ├── linkmarks-bridge-pinboard/  # bin — Pinboard API (Fase 2+)
│       └── linkmarks-bridge-linkwarden/ # bin — Linkwarden API (Fase 2+)
└── docs/
    ├── ARCHITECTURE.md                 # this file
    ├── SPEC.md
    ├── ROADMAP.md
    └── CONCERNS.md
```

Each bridge is its own crate because they have heterogeneous deps
(Chromium = serde only, Firefox = rusqlite + jsonlz4 decompression,
Pinboard = reqwest + API key, Linkwarden = reqwest + their types).
Co-locating them in one crate would inflate build matrix.

## Crate responsibilities

### `linkmarks-core` (lib)

Pure-Rust, no I/O deps beyond parsing helpers. Provides:

- `model.rs` — normalized domain types. See **Domain model** below.
- `traits.rs` — `BookmarkSource` and `BookmarkSink` traits. See **Traits** below.
- `canonical.rs` — URL canonicalization (lowercase host, strip trailing
  slash, sort query params, drop tracking params like `utm_*`,
  resolve redirects on demand). Deterministic, side-effect-free.
- `dedupe.rs` — local dedupe by canonical URL. Reports conflicts
  (same canonical URL, different titles/tags) instead of silently merging.
- `parser/` — shared helpers (HTML entities, CSV, MIME sniffing).

**Invariants**:
- Never owns I/O (no `std::fs`, no `reqwest`).
- Never owns time (clock injection via trait for testability).
- All public types `#[serde]`-friendly so bridges and server can
  serialize without extra adapters.

### `linkmarks-cli` (bin)

Thin wrapper around `linkmarks-core` for headless usage.
- `clap` v4 derive for subcommand parsing.
- Output formats: `table` (default, deterministic column order), `json`
  (one bookmark per line NDJSON), `yaml`. Stable field order.
- Exit codes: 0 OK, 1 partial / source error, 2 invalid args, 3 dedupe
  conflicts found (non-fatal, request from operator).

### `linkmarks-tui` (bin, Fase 2)

Interactive browse + dedupe review.
- `ratatui` + `crossterm`. Single-threaded, event loop.
- Reads normalized model from a local store (sqlite file or stdin NDJSON).
- Does NOT own the store; defers to `linkmarks-core`.

### `linkmarks-gui` (bin, Fase 4)

Dioxus desktop app.
- Same model + traits as CLI. Adds visualization, sharing, drag-drop.
- Local-first: works offline; syncs to server when reachable.
- Dioxus chosen for native rendering, single binary distribution, and
  headless testability via a dedicated `lzt-test-engineer` style agent.

### `linkmarks-server` (bin, Fase 3)

Relay for CRDT sync between clients.
- `axum` HTTP + WebSocket.
- `sqlx` for SQLite persistence (bookmarks metadata, sync state, audit).
- `yrs` for CRDT document storage (per-user y-doc blobs).
- Auth via API key header (v1), no account system on server side.
- Server is **optional**. Clients work fully offline; server is relay-only.

### Bridges (`linkmarks-bridge-*`)

Each bridge implements `BookmarkSource` and/or `BookmarkSink` from
`linkmarks-core`. Format-specific I/O lives here. Bridges never
implement dedupe or canonicalization logic — that stays in core.

**v1 must-have bridges** (Fase 1-2):
- `bridge-chromium` — read `Bookmarks` JSON file.
- `bridge-firefox` — read `places.sqlite` (read-only mode; no profile
  lock contention) + `bookmarks-backups/*.jsonlz4`.
- `bridge-netscape` — read/write Netscape HTML format (universal
  interchange).

**v2 bridges** (Fase 2+, gated on operator approval):
- `bridge-pinboard` — Pinboard API v1 (`posts/all`, `posts/add`).
- `bridge-linkwarden` — Linkwarden REST API.

## Domain model (`linkmarks-core`)

```rust
pub struct Bookmark {
    pub id: BookmarkId,                  // ULID, server-assigned or local UUID
    pub original_url: String,            // raw URL as imported (preserved)
    pub canonical_url: String,           // normalized; dedupe key
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,               // sorted, lowercase, dedup'd
    pub collection: Option<String>,      // folder path, "/"-separated
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source: SourceRef,               // provenance
    pub content_type: Option<String>,    // sniffed if available
    pub archived: bool,
}

pub struct SourceRef {
    pub kind: SourceKind,                // Chromium, Firefox, Pinboard, Manual
    pub external_id: Option<String>,     // provider-side id
    pub imported_at: DateTime<Utc>,
    pub raw: Option<serde_json::Value>,  // original payload, for audit
}

pub enum SourceKind {
    Chromium,
    Firefox,
    Netscape,
    Pinboard,
    Linkwarden,
    Manual,
}

pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub parent: Option<CollectionId>,
    pub source: SourceKind,
}

pub struct Tag(pub String);              // newtype, validation in FromStr
```

**URL preservation**: `original_url` is **never** rewritten. Even if
the canonical form differs (e.g. `HTTPS://Example.com/?utm_source=x`),
the original is preserved verbatim for round-trip fidelity. This is a
hard rule — see CONCERNS.md.

## Traits (`linkmarks-core`)

```rust
pub trait BookmarkSource: Send + Sync {
    fn kind(&self) -> SourceKind;
    fn list(&self) -> Result<Vec<Bookmark>>;
    fn list_paginated(&self, cursor: Option<String>, limit: usize) -> Result<Page>;
    fn by_canonical(&self, canonical: &str) -> Result<Option<Bookmark>>;
}

pub trait BookmarkSink: Send + Sync {
    fn kind(&self) -> SourceKind;
    fn write(&mut self, bookmarks: &[Bookmark]) -> Result<WriteReport>;
    fn delete(&mut self, external_id: &str) -> Result<()>;
}
```

The CLI and server orchestrate these via
`SourceRegistry { sources: Vec<Box<dyn BookmarkSource>> }`. Bridges
register themselves at startup. Plugins (Fase 5) implement the same
traits via a dynamic loader (`libloading`) gated on a manifest schema.

## Open architecture questions

These are tracked in CONCERNS.md and require operator input:

1. CRDT choice: `yrs` (Y-CRDT) vs `automerge-rs`. `yrs` is provisional,
   spike in Fase 3.
2. Server deployment: single binary vs Docker image vs systemd unit.
   CLI-first; Docker is optional and post-MVP.
3. Plugin ABI stability: when to lock the `BookmarkSource` trait version?
   Suggest: lock at v1.0.0 of `linkmarks-core`, before Fase 5 plugin
   market opens.
4. Multi-account sync: is the server per-user or per-workspace?
   Provisional: per-user, single workspace per user.
5. CRDT document granularity: one y-doc per user vs per collection.
   Provisional: per user, with collection-level subdocs.
