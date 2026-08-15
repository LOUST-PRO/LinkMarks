# Changelog — LinkMarks

All notable changes to this project are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/) and the
project adheres to [Semantic Versioning](https://semver.org/).

## [2.0.1] — 2026-08-15

Patch release — Dependabot `rust-security` group bump + compat fixes.

### Changed (dependency upgrades, no breaking API)
- `thiserror` 1.0.69 → **2.0.20** (major)
- `ulid` 1.2.1 → **3.0.0** (major)
- `rusqlite` 0.31.0 → **0.40.2** (minor)
- `clap` 4.6.4 → 4.6.6 (patch)
- `quick-xml` 0.36.2 → **0.41.0** (minor)
- `lz4_flex` 0.11.6 → 0.14.0 (minor)
- `dirs` 5.0.1 → **6.0.0** (major)
- `toml` 0.8.23 → **1.1.4+spec-1.1.0** (major)
- `ratatui` 0.28.1 → 0.30.2 (minor)
- `crossterm` 0.28.1 → 0.29.0 (minor)

### Fixed (compat with upgraded deps)
- **ulid 1.x → 3.x**: `Ulid::new()` removed in 3.x; the
  now-constructor is `Ulid::generate()` (returns `Self`, not `Result`).
  Updated single call site in
  `crates/linkmarks-core/src/model.rs:23` (`BookmarkId::generate`).
- **quick-xml 0.36 → 0.41**: character references (`&amp;`, `&#160;`,
  `&aacute;`, …) are now emitted as a separate `Event::GeneralRef`
  instead of being folded into the surrounding `Event::Text`. Added
  the missing exhaustive-match arm in
  `crates/bridges/linkmarks-bridge-netscape/src/parser.rs` that
  decodes the entity body through the same `resolve_entity` /
  `decode_numeric_entity` helpers the text path uses.
  - Behavioural correction fell out of this work: previously both
    `handle_text` and the new `GeneralRef` arm inserted a single
    ASCII space as a 'separator' before each non-empty text chunk.
    With quick-xml 0.36 that was a no-op (single self-contained
    chunks); with 0.41 a logical text chunk like `AT&amp;T` becomes
    `Text("AT") + GeneralRef(&) + Text("T")`, and the separator
    logic produced `"AT & T"` instead of the expected `"AT&T"`.
    The fix is to drop the separator: the contents of a `<A>` title
    is one contiguous string.
  - `attr.unescape_value()` is deprecated in quick-xml 0.41 in
    favour of `normalized_value()`. The replacement additionally
    applies XML attribute-value normalization (whitespace
    collapsing), which would corrupt URLs that legitimately contain
    multiple spaces; we pin to `unescape_value()` with
    `#[allow(deprecated)]` and document the rationale inline.
- **clippy `explicit-auto-deref`** under Rust 1.97: replaced
  `&**text` with the cleaner `let raw_bytes: &[u8] = text;` (Deref
  coercion handles the rest).

### Test surface
- Total tests: **250 / 250 green** (`cargo test --workspace --no-fail-fast`).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo build --workspace --all-targets` clean.
- `cargo fmt --all -- --check` clean.
- CI smoke (`fmt + build + test + clippy + release binary + groff
  manpage`) green on PR #10.

### Anti-features (locked — unchanged from v2.0.0)
- No telemetry, no phoning home, no automatic update notifier.
- No server-authoritative mode.
- No AI-without-cost-gate.
- No Docker-only deploy.
- No closed-source build.

## [2.0.0] — 2026-08-13

Fase 2 batch — interactive TUI + Firefox import + SQLite store.

### Added
- **`linkmarks-core`**: SQLite store (`rusqlite`, WAL mode, busy_timeout
  5s, foreign_keys=ON, soft-archive via `archived_at`), XDG path
  resolution (`dirs`), TOML config loader, schema migrations runner
  (4 statements: store init, indices, view v_bookmarks_full, pragmas).
- **`linkmarks-tui`** (new crate, ~2159 LOC): ratatui + crossterm-based
  interactive terminal browser. Browse collections, filter by substring
  / tag, view metadata, open URLs. 71 tests across 4 integration suites.
- **`linkmarks-bridge-firefox`** (new crate, ~873 LOC): read-only
  `places.sqlite` reader (moz_bookmarks / moz_places / moz_tags), raw
  LZ4 block decompressor for `bookmarks-backups/*.jsonlz4` (mozLz40\0
  prefix + `lz4_flex` block). 5 modules, 5 in-crate tests.
- **`linkmarks-bridge-netscape`** (~2181 LOC, already shipped): full
  Netscape bookmark HTML parser + sink with custom HTML5 named-entity
  decoder (~40 entities) + numeric character reference handling.
  Atomic write via tempfile + persist. 26 in-crate tests +
  integration tests.
- **`scripts/install.sh`**: bash, `set -euo pipefail`. Auto-detects
  host. Options: `--prefix`, `--binary-from`, `--force`, `--dry-run`,
  `--help`. Validates installed binary with `--version` + `--help`.
- **`docs/man/linkmarks.1`**: 358-line roff `-man` page covering NAME,
  SYNOPSIS, DESCRIPTION, SUBCOMMANDS, EXIT STATUS, ENVIRONMENT, FILES,
  EXAMPLES, SEE ALSO. Renders cleanly with `groff -man`.
- **`.github/workflows/ci-smoke.yml`**: 2 jobs (`smoke` + `manpage`)
  on `ubuntu-latest` running fmt + build + test + clippy + release
  binary smoke + groff render. Triggers on `feat/**`, `fix/**`,
  `chore/**`, `main`, `master`, and PRs to main/master.
- **`tests/install_test.sh`**, **`tests/manpage_test.sh`**: bash smoke
  scripts for the install path and manpage rendering.

### Changed
- **Workspace version bump 1.0.1 → 2.0.0** — major bump because
  Fase 2 introduces a new persistence surface (SQLite store, was
  in-memory only at v1.0.x) and the new `tui` subcommand changes
  the CLI surface.
- `linkmarks-cli` gains a `tui` subcommand (`crates/linkmarks-cli/src/cmd/tui.rs`).
- All crates continue to inherit `version.workspace = true`, so a single
  bump propagates across 6 crates.
- Workspace member list now includes `crates/linkmarks-tui`,
  `crates/bridges/linkmarks-bridge-firefox`.

### Test surface
- Total tests: **250 / 250 green** (`cargo test --workspace --no-fail-fast`).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo build --workspace --all-targets` clean.

### Anti-features (locked — unchanged from v1.0.0)
- No telemetry, no phoning home, no automatic update notifier.
- No server-authoritative mode.
- No AI-without-cost-gate.
- No Docker-only deploy.
- No closed-source build.

### Deferred (F2.5 follow-up)
- **Fuzzy filter via `nucleo`** — implementation present in working tree
  (`crates/linkmarks-tui/src/filter.rs`, sort.rs, filter_fuzzy_test.rs) on
  branch `chore/snapshot-2026-08-15-fuzz-foreign-wt` (0f5b6cc). Tracked
  for the F2.5 PR alongside sort modes.
- **Sort modes** — sort.rs preserved on the snapshot branch.
- **Shell completions** — `clap_complete` + build script. Out of scope
  per F5 PR body.
- **README Install / Subcommands / CI sections refresh** — original v1
  install instructions remain. Tracked for a separate docs pass.
- **Debian / RPM / Arch / Homebrew packaging** — out of scope, follows
  in a future maintenance release once install.sh stabilizes.
- **`cargo fmt --check` 56 hunks of pre-existing drift** — non-F5 drift,
  to be batched with F2.5.

### Planned (see ROADMAP.md)
- Fase 3: CRDT sync server (spike `yrs` vs `automerge-rs` first), opt-in
  `linkmarks sync --server <url>`.
- Fase 4: `linkmarks-gui` (Dioxus desktop).
- Fase 5: Plugin market with stable ABI + signed binaries.

## [1.0.1] — 2026-08-03

### Changed
- **Tracking-param blocklist expanded** — added `mc_eid`, `mc_cid`,
  `igshid`, `ref_src` to the canonical-URL stripping set (was 5 → 9
  params). Documented in `docs/decisions/0004-tracking-params.md`.
- **SQLite WAL storage** — `linkmarks-core` storage layer now uses
  WAL journal mode + `busy_timeout=5000` for concurrent reader/writer
  safety. Migration is idempotent (`PRAGMA journal_mode=WAL` re-issued
  on every connection).
- **3 ADRs added** — `0002-dioxus-for-gui.md`,
  `0003-sqlite-as-store.md`, `0004-tracking-params.md`.

## [1.0.0] — 2026-07-28

### Added
- Workspace skeleton: `linkmarks-core`, `linkmarks-cli`, `linkmarks-bridge-chromium`.
- `linkmarks-core` with normalized `Bookmark`, `SourceRef`, `Collection`,
  `Tag` model and `BookmarkSource` / `BookmarkSink` traits.
- URL canonicalization rules (ADR-0001) with tracking-param blocklist
  (`utm_*`, `fbclid`, `gclid`, `ref`, `ref_src`, `mc_eid`, `mc_cid`).
- Local deterministic dedupe by canonical URL with conflict report.
- `linkmarks-cli` with `list`, `import`, `export`, `dedupe` subcommands
  and `table` / `json` / `yaml` output formats.
- `linkmarks-bridge-chromium` parser for Chromium-family `Bookmarks`
  JSON (Chrome, Brave, Edge, Arc, Vivaldi, Opera).
- Fixture corpus: 5-bookmark anonymized example at
  `crates/bridges/linkmarks-bridge-chromium/tests/fixtures/chrome-bookmarks.example.json`.
- ADRs:
  - `0001-licensing.md` — AGPLv3 + Commercial dual.
- Exit codes: 0 OK, 1 partial, 2 invalid args, 3 dedupe conflicts.

### Anti-features (locked)
- No telemetry, no phoning home, no automatic update notifier.
- No server-authoritative mode.
- No AI-without-cost-gate.
- No Docker-only deploy.
- No closed-source build.

### Planned (see ROADMAP.md)
- Fase 2: `linkmarks-tui` (ratatui), `linkmarks-bridge-firefox`,
  multi-source import.
- Fase 3: CRDT sync server (`yrs` vs `automerge-rs` spike first),
  `linkmarks sync --server <url>`.
- Fase 4: `linkmarks-gui` (Dioxus desktop).
- Fase 5: Plugin market with stable ABI and signed binaries.
