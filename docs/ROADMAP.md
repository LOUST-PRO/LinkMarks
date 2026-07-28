# LinkMarks — Roadmap

**Status**: Documentation staging. No code yet.
**Date**: 2026-07-28

5 phases. Each phase is shippable independently. No phase depends on a
later phase's deliverables. A phase's exit criteria must pass before the
next phase starts.

The numbering reflects dependency order, not priority — a phase can be
shipped as the "MVP" while the next phases are still in design.

## Fase 1 — Visible MVP

**Goal**: a usable CLI that imports Chromium bookmarks, lists them
deterministically, and exports to Netscape HTML. The minimum thing a
power user can replace Raindrop with, headlessly.

**Scope**:
- Workspace skeleton (Cargo.toml, crate dirs, CI scaffold).
- `linkmarks-core` with `Bookmark`, `Collection`, `BookmarkSource`,
  `BookmarkSink` traits, URL canonicalization, local dedupe.
- `linkmarks-cli` with `list`, `import`, `export`, `dedupe` subcommands.
- `linkmarks-bridge-chromium` implementing `BookmarkSource` for the
  Chromium `Bookmarks` JSON.
- `linkmarks-bridge-netscape` implementing both `BookmarkSource` and
  `BookmarkSink` for Netscape HTML.
- Local store: SQLite via `rusqlite`. Single file, default path
  `~/.local/share/linkmarks/store.db` (override via `--store`).
- Fixture corpus (10+ real Chromium exports, anonymized, committed
  under `tests/fixtures/chromium/`).

**Dependencies**: Rust 1.78+, `cargo`, `clap` v4, `serde`, `rusqlite`,
`url` crate, `chrono`, `ulid`, `anyhow`/`thiserror`.

**Deliverables**:
- All 5 v1 must-have features (per SPEC.md) functional and tested.
- `linkmarks list --source=chrome` exits 0 on the fixture corpus.
- Dedupe dry-run produces byte-identical report across 3 runs.

**Exit criteria**:
1. All 5 v1 features pass acceptance (SPEC.md §Acceptance criteria).
2. `cargo test --workspace` green.
3. CI smoke test: `linkmarks import tests/fixtures/chromium/x.json &&
   linkmarks list --source=chrome | wc -l` matches expected count.
4. No network calls in CLI smoke (strace verified).
5. README + CHANGELOG.md for v0.1.0.

**Not in scope**: Firefox, TUI, GUI, server, sync, anything beyond v1.

## Fase 2 — Interactive CLI

**Goal**: a ratatui TUI for browsing, dedupe conflict review, and
multi-source import. Firefox import lands here.

**Scope**:
- `linkmarks-tui` crate (ratatui + crossterm).
- `linkmarks-bridge-firefox` — read `places.sqlite` read-only + parse
  `bookmarks-backups/*.jsonlz4`. Two crates in one: the SQLite reader
  and the jsonlz4 decompressor share a helper crate.
- Multi-source `linkmarks import` (accept `--source=chrome,firefox`).
- Dedupe conflict resolution UI in TUI (apply / skip / inspect).
- Source plugins registered via `--source` flag (not dynamic loading
  yet — that's Fase 5).

**Dependencies**: Fase 1 complete. `ratatui`, `crossterm`, `jsonlz4`
(or hand-rolled decompressor), `rusqlite` for places.sqlite.

**Deliverables**:
- TUI launchable via `linkmarks tui`.
- Firefox import working on a real profile copy.
- Dedupe conflicts resolvable interactively.

**Exit criteria**:
1. TUI launches, browses 10k bookmarks without UI lag (60 fps target).
2. Firefox import of a real profile produces correct canonical URLs.
3. Multi-source import de-dupes across sources deterministically.
4. `cargo test --workspace` green.

**Not in scope**: Pinboard bridge, Linkwarden bridge, GUI, server,
sync. These move to Fase 3+.

## Fase 3 — CRDT sync (server optional)

**Goal**: optional relay server, multi-device sync via CRDT. Local
mode unchanged — server is opt-in.

**Scope**:
- **Spike first**: 1-week POC comparing `yrs` (Y-CRCT) vs
  `automerge-rs`. Decision documented in
  `docs/decisions/03-crdt-choice.md` with measured numbers
  (encode size, op latency, memory per 1k bookmarks).
- `linkmarks-server` crate — axum HTTP + WebSocket, sqlx (SQLite
  for metadata, file blob for y-doc snapshots).
- `linkmarks-core` gains a `SyncAdapter` trait wrapping the chosen
  CRDT.
- Auth: API key header (v1). No email/password, no OAuth, no JWT.
- Audit log: every sync emits a row to the server's audit table.
- `linkmarks sync --server <url>` subcommand.

**Dependencies**: Fase 2 complete. `axum`, `tokio`, `sqlx`, `yrs`
(or `automerge-rs`), `tokio-tungstenite`.

**Deliverables**:
- Spike report committed.
- Server binary runs as single process, ~50 MiB RSS idle.
- Two clients sync via server, conflict-free convergence verified by
  integration test.

**Exit criteria**:
1. Spike decision committed with evidence.
2. Server passes `cargo test` including integration tests with two
   simulated clients.
3. Server binary runs in CI smoke (systemd unit OR Docker; pick one,
   document the choice).
4. Sync round-trip: A edits, B receives edit, no data loss across 100
   randomized concurrent edits.

**Not in scope**: GUI, plugin market, hosted SaaS UI. These are Fase 4+.

**Risk note**: if `yrs` vs `automerge-rs` POC shows >2× difference on
either dimension (size or latency), the spike decision can revisit
provisional `yrs` choice. Cost: 1 week of Fase 3 timeline.

## Fase 4 — Dioxus GUI

**Goal**: desktop GUI for visualization, sharing, drag-drop. Same data
model, new presentation.

**Scope**:
- `linkmarks-gui` crate — Dioxus desktop (decision recorded in
  ADR 0002; see `docs/decisions/0002-dioxus-for-gui.md`).
- Drag-drop import (drop a Chromium JSON into the window).
- Sharing view: read-only link to a collection via the server (Fase 3
  server provides this endpoint).
- Local search (substring match, no embeddings — see SPEC.md §5).

**Dependencies**: Fase 3 complete. Dioxus 0.6+ (or whichever is stable
at the time), `dioxus-desktop`.

**Deliverables**:
- Desktop app builds and runs on Linux (other platforms: stretch).
- Same local store, same CLI commands work alongside GUI.
- GUI never requires the server (offline-first).

**Exit criteria**:
1. GUI launches on a fresh machine without errors.
2. Drag-drop of a Chromium JSON imports correctly (same canonical
   URLs as CLI).
3. GUI doesn't crash on empty store.
4. No GUI-only state divergence from CLI store (round-trip via GUI +
   `linkmarks list` shows same data).

**Not in scope**: mobile native, browser extension, web UI. The
SaaS/web UI is a separate product (see CONCERNS.md).

## Fase 5 — Plugin market

**Goal**: third-party bridges publishable as standalone crates, loaded
via a stable ABI. Manifest schema for plugin metadata.

**Scope**:
- Lock `linkmarks-core` trait version at v1.0.0 (semver-stable).
- Plugin manifest schema (`plugin.toml`): name, version, source/sink
  kinds, capability declaration, license.
- `linkmarks plugin install <name>` subcommand — fetches plugin
  binaries (signed? — see CONCERNS.md §plugin security).
- `libloading`-based loader, sandboxed by process isolation (each
  plugin runs in its own subprocess).

**Dependencies**: Fase 4 complete. `libloading`, plugin registry format
(TBD — likely a flat namespace under `louzt/linkmarks-plugin-*`).

**Deliverables**:
- Stable plugin ABI spec committed.
- 2 example third-party plugins (e.g. Reddit-saved, GitHub-stars) to
  validate the ABI.
- Plugin registry metadata schema.

**Exit criteria**:
1. ABI versioned, semver-stable, documented.
2. At least 2 non-core plugins published and installable.
3. Plugin install is reversible (`linkmarks plugin remove`).
4. No plugin gets network access without an explicit `--allow-net`
  flag at install time.

**Not in scope**: paid plugin marketplace, plugin analytics, plugin
ratings. These are not LinkMarks' product surface.

## Out of all phases (explicit never-do)

- Mobile native app — web responsive UI is the long-term answer.
- OCR / read-it-later — LinkMarks is a bookmark manager.
- AI embeddings / semantic search without explicit cost gate per
  action — anti-feature per SPEC.md.
- Mandatory telemetry / opt-out analytics — anti-feature.
- Closed-source build / obfuscated binaries — anti-feature.

## Phase dependencies summary

```
Fase 1 (CLI MVP)        → shippable
Fase 2 (TUI + Firefox)  → depends on Fase 1
Fase 3 (CRDT sync)      → depends on Fase 2; spike first
Fase 4 (Dioxus GUI)     → depends on Fase 3
Fase 5 (Plugin market)  → depends on Fase 4
```

Each phase's exit criteria are a hard gate. Skipping a phase's exit
review to start the next phase is a CONCERNS.md entry, not a default.
