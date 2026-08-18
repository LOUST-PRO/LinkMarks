# linkmarks-core

Core domain model, traits, URL canonicalization, and local dedupe primitives
for the LinkMarks workspace. This is the foundation that every other
LinkMarks crate builds on; it has no CLI or TUI surface of its own.

## What is it

`linkmarks-core` is the shared vocabulary and storage layer for a
local-first bookmark manager. It exposes:

- A `Bookmark` model (id, canonical URL, title, source, timestamps, tags).
- A `BookmarkSource` trait that all bridges implement.
- A `Store` API backed by SQLite (WAL mode, ulid primary keys).
- URL canonicalization rules (lowercase host, drop default ports, strip
  tracking parameters, normalise path).
- Deterministic dedupe hashing for the `dedupe` subsystem.
- Path resolution under XDG Base Directory, overridable via
  `LINKMARKS_STORE` / `LINKMARKS_CONFIG` or the `--store` / `--config`
  flag.

The `Store` is hand-rolled; schema migrations live under `src/migrations/`
and are applied in sorted order at startup. The on-disk format is
documented alongside the source.

## Install

This crate is a library. It is published as part of the LinkMarks
workspace; you usually do not depend on it directly unless you are
writing a new bridge or a new surface on top of the same store.

```toml
[dependencies]
linkmarks-core = "2.2.0"
```

To build from source:

```bash
git clone https://github.com/LOUST-PRO/LinkMarks
cd LinkMarks
cargo build -p linkmarks-core
```

## Usage

```rust
use linkmarks_core::{Bookmark, BookmarkSource, Store, Paths};

let paths = Paths::resolve_default()?;
let store = Store::open(&paths.store)?;
let list: Vec<Bookmark> = store.list_all()?;
```

A full worked example (open the store, iterate, dedupe, write back) lives
in the umbrella `linkmarks` crate's CLI subcommands.

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md` at the project root. Contact:
`opensource@loust.pro`.
