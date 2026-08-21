# Introduction

**LinkMarks** is a local-first bookmark manager with deterministic
deduplication, multi-device sync over CRDT, and an interactive TUI
browser. It imports what you already have (Chromium, Firefox,
Netscape HTML exports), deduplicates by canonical URL, keeps
everything in a single SQLite store under `~/.local/share/linkmarks/`,
and optionally syncs across your devices through a self-hosted
yrs-based relay.

## TL;DR

```bash
# Install
cargo install linkmarks-cli --path crates/linkmarks-cli --bin linkmarks --locked

# Import from a browser export
linkmarks init
linkmarks import chromium ~/.cache/chromium/Default/Bookmarks

# Browse interactively
linkmarks tui

# Or script it
linkmarks list --format json | jq '.[] | select(.tags | index("rust"))'
```

## What it is

LinkMarks treats bookmarks as first-class Rust data with five
properties that matter:

1. **Local-first.** The SQLite store is the source of truth. Sync is
   additive, never required for correctness. A user who never runs
   `linkmarks sync` has the same data as a user with three devices.
2. **Deterministic.** Dedup is by canonical URL (sorted query
   params, lowercased host, no fragment, no trailing slash variance).
   Re-running `linkmarks dedupe` against the same input produces
   byte-identical output.
3. **Auditable.** The codebase is 8 crates in a Cargo workspace. The
   core (`linkmarks-core`) is a library with no I/O outside the
   SQLite handle the caller hands it. The CLI (`linkmarks-cli`) is
   a thin clap wrapper. There is no telemetry, no analytics, no
   automatic network call.
4. **Multi-format import.** Chromium's `Bookmarks` JSON, Firefox's
   `places.sqlite`, and Netscape-style HTML exports are all parsed
   natively. Round-trip fidelity is verified by unit tests for
   each bridge.
5. **Multi-device via yrs CRDT.** When the operator chooses to run
   the self-hosted relay (preview), changes propagate as
   per-collection yrs sub-documents. Conflicts merge without
   operator action; the relay sees opaque bytes, not plaintext
   bookmarks.

## What it is not

- **A bookmarking service.** There is no central account. There is
  no SaaS. There is no "free tier" that costs you your privacy.
- **A web clipper.** LinkMarks is for URLs, titles, tags, and
  notes. Not for full-page archives, screenshots, or rich text.
- **A read-it-later service.** The TUI is a browser, not a reader.
  It does not download page contents.

## Who it's for

LinkMarks is for operators who:

- Care about the difference between `m.example.com` and
  `example.com` (canonical URL dedup handles this)
- Run multiple devices and want bookmarks synced without
  trusting a third-party SaaS with the data
- Like a TUI as their primary surface (ratatui + nucleo fuzzy
  search)
- Want to be able to read every line of code that touches their
  bookmarks (the core is ~3,500 LOC, the whole workspace is ~12,000
  LOC excluding `linkmarks-bench-crdt`)

## Crate map

The workspace is 8 crates; the umbrella `linkmarks` is the
canonical binary:

| Crate | Visibility | Description |
|---|---|---|
| `linkmarks` | public (umbrella) | Single-binary that re-exports the CLI |
| `linkmarks-cli` | public | The clap-based command dispatch |
| `linkmarks-core` | public | Library: SQLite schema, dedup, sort, filter |
| `linkmarks-tui` | public | Interactive ratatui browser |
| `linkmarks-bridge-chromium` | public | Chromium `Bookmarks` JSON parser |
| `linkmarks-bridge-firefox` | public | Firefox `places.sqlite` parser |
| `linkmarks-bridge-netscape` | public | Netscape HTML parser/serializer |
| `linkmarks-bench-crdt` | private | Benchmark harness (not published) |

Every public crate ships to [crates.io](https://crates.io/crates/linkmarks-core)
with the same `Cargo.lock` pinned via `--locked` install. The umbrella
binary is documented at
[docs.rs/linkmarks](https://docs.rs/linkmarks); the library API
is documented at the per-crate docs.rs page.

## At a glance

| Property | Value |
|---|---|
| Language | Rust (edition 2021, MSRV 1.78) |
| License | AGPL-3.0-or-later OR LicenseRef-Commercial |
| Storage | SQLite (single file under XDG) |
| Sync | yrs CRDT, optional self-hosted relay (preview) |
| Bridges | Chromium JSON, Firefox `places.sqlite`, Netscape HTML |
| CLI | `linkmarks` (single binary) |
| TUI | ratatui + crossterm + nucleo |
| Runtime deps | 16 per crate (workspace-shared `Cargo.lock`) |
| Test count | 286 (workspace, release mode) |
| Headline LOC | ~12,000 across 8 crates (excluding bench) |

## Where to next

- [Getting started](./installation.md) — `cargo install` and the
  first `linkmarks import`.
- [Concepts](./concepts.md) — the canonical URL model, the SQLite
  schema, the dedup algorithm.
- [CLI reference](./cli.md) — every subcommand with flags.
- [TUI browser](./tui.md) — keys, sort modes, filter modes.
- [Bridge formats](./bridges.md) — what's preserved per import.
- [Sync model](./sync.md) — how the relay sees your bookmarks.
- [Architecture](./architecture.md) — the 8-crate workspace
  breakdown.
- [Hardening](./hardening.md) — operational hardening the
  reference deployment applies.
- [Reference](./reference.md) — env vars, exit codes, file
  layout.
- [License](./license.md) — AGPL-3.0-or-later OR
  LicenseRef-Commercial.