# Changelog — LinkMarks

All notable changes to this project are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/) and the
project adheres to [Semantic Versioning](https://semver.org/).

## [2.2.0] — 2026-08-17

Minor release — umbrella refactor + dual lib+bin target for the CLI.

### Added
- **Umbrella crate `linkmarks`.** A new top-level workspace member at
  `crates/linkmarks` collapses the seven previously-individual
  crates (`linkmarks-core`, `linkmarks-cli`, `linkmarks-tui`, and
  the three browser bridges) into one crates.io artifact. End users
  now install the CLI with a single `cargo install linkmarks --locked`
  instead of juggling a `--path crates/linkmarks-cli --bin linkmarks`
  recipe.
- **Dual lib+bin target on `linkmarks-cli`.** The CLI crate is now
  both a library (exposing `linkmarks_cli::run()` for the umbrella
  binary and any future embedding) and a binary (preserving the
  existing `cargo install --path crates/linkmarks-cli --bin linkmarks`
  workflow for users who build directly from the workspace).
- **`linkmarks` umbrella binary.** `crates/linkmarks/src/main.rs` is
  an 8-line wrapper that calls `linkmarks_cli::run()` and translates
  the `Result<i32>` exit code into a `std::process::exit` value.
- **Workspace re-exports.** `linkmarks::lib.rs` re-exports
  `linkmarks_core` (domain model, storage, canonicalization, dedupe)
  and `linkmarks_tui` (interactive terminal browser), so downstream
  Rust consumers can `use linkmarks::{core, tui}` instead of carrying
  six separate `Cargo.toml` dependencies.

### Changed
- **`linkmarks-cli` visibility.** Sub-modules `cmd` and `ui` are now
  `pub mod` (were private `mod`), and the `Format` enum is `pub`
  (was private). Sub-module reachability is required for the umbrella
  binary to delegate to `linkmarks_cli::run()` without breaking the
  `cmd::*::run` and `ui::render` signatures.
- **Root `README.md` Install section.** Adds the `cargo install
  linkmarks --locked` recipe (umbrella from crates.io) and updates
  the `cargo install --git` recipe to point at `crates/linkmarks` and
  `--tag v2.2.0`.
- **Workspace version bump.** `workspace.package.version` advances
  from `2.1.2` to `2.2.0`. All seven sub-crates inherit the bump via
  `version.workspace = true`.

### Removed
- **No crates.io pages for the seven sub-libraries.** Each
  sub-crate's `Cargo.toml` gains `publish = false`. They remain in
  the workspace and are bundled into the umbrella binary, but no
  longer produce their own crates.io artifacts. The `linkmarks`
  umbrella is the single published crate for this project.

### Notes
- **No behavior change.** The CLI surface (subcommands, flags, exit
  codes, output formats) is identical to v2.1.x. The umbrella is a
  packaging refactor; the underlying engines are unchanged.
- **License unchanged.** SPDX `AGPL-3.0-or-later OR
  LicenseRef-Commercial` (per `dual-license-OR-not-AND.md`).

## [2.1.1] — 2026-08-15

Patch release — wiring + packaging + doc drift fixes for v2.1.0.

### Fixed
- **TUI: filter & sort enums now fully reachable.**
  `FilterMode::Fuzzy` was declared but only reachable via direct
  constructor; press `Ctrl+F` inside the filter to cycle
  `Substring → Tag → Fuzzy`. `SortMode` gains a 4th variant
  `CanonicalUrl` and a global `s` keybinding to cycle the four
  modes (`updated / title / url / created`). The status bar now
  surfaces the active `sort: <label>` and the help overlay lists
  both new bindings.
- **`debian/rules`** — replaced deprecated `dh $@ --with cargo`
  with `dh $@ --buildsystem cargo` and made
  `override_dh_auto_test` honor `DEB_BUILD_OPTIONS=nocheck`
  instead of unconditionally skipping the suite.
- **`arch/PKGBUILD`** — switched the install hook from a
  hand-copied `/usr/share/libalpm/scripts/` copy to the
  canonical `install=linkmarks.install` directive, and
  regenerated `.SRCINFO`. The downstream maintainer no longer
  has to apply a namcap-flagged pattern.
- **`rpm/linkmarks.spec`** — `License: AGPL-3.0-or-later AND
  LicenseRef-Commercial` → `AGPL-3.0-or-later OR
  LicenseRef-Commercial` (dual-license semantics, SPDX-compliant
  per `dual-license-OR-not-AND.md`).
- **`README.md`** — `cargo install --git … --bin linkmarks` →
  `… --path crates/linkmarks-cli --bin linkmarks --locked`
  (workspace path resolution per
  `cargo-workspace-install-recipe.md`); corrected the
  `[CONTRIBUTING](./.github/PULL_REQUEST_TEMPLATE.md)` label
  mismatch (now `[PR template](./.github/PULL_REQUEST_TEMPLATE.md)`);
  added the missing `text` language hint to the project-layout
  fenced block (`code-fence-language-required.md`).
- **`arch/README.md`** — fixed `-skipchecksums` → `--skipchecksums`.

### Added
- **9 state-wiring integration tests** in
  `crates/linkmarks-tui/src/app_test.rs` covering every
  `SortMode` and `FilterMode` variant via emulated `KeyEvent`s
  and asserting the downstream `app.visible()` effect — per
  `state-wiring-integration-tests.md`.
- **6 sort tests** in `crates/linkmarks-tui/src/sort.rs`:
  `all_constant_matches_next_cycle`, `canonical_url_asc_is_case_insensitive`,
  `created_desc_orders_newest_first`, `ties_break_by_id_for_full_determinism`,
  and the parity checks for the cycle.
- **4 input-handler tests** in `crates/linkmarks-tui/src/input.rs`:
  `s_cycles_sort_mode`, `ctrl_f_cycles_filter_mode`,
  `ctrl_f_outside_filter_is_a_no_op`, `ctrl_f_preserves_query`.

### Hardened (CodeRabbit → `.claude/rules/`)
- 10 new `~/.claude/rules/*.md` snippets codified from this
  review (`packaging-template-vs-reality`, `cargo-workspace-install-recipe`,
  `dual-license-OR-not-AND`, `code-fence-language-required`,
  `doc-link-label-must-match-target`, `dh-buildsystem-syntax`,
  `documented-api-must-match-implementation`,
  `reachable-feature-enum-keybinding-state-test`,
  `pacman-install-directive-not-libalpm-copy`,
  `debian-test-override-respects-DEB_BUILD_OPTIONS`).
- 3 more from mid-turn hardening dialogue
  (`state-wiring-integration-tests`, `pr-body-vs-diff-literal`,
  `packaging-linter-mandatory`).

### Test surface
- Total tests: **302 / 302 green** (was 286 in v2.1.0).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo build --release` clean.
- `cargo fmt --all -- --check` clean.
- `bash -n arch/PKGBUILD` clean.
- `make -n -f debian/rules build` clean.
- `rpmspec -P rpm/linkmarks.spec` clean (License OR).
- Completions smoke (bash/zsh/elvish) clean.

### Linter gap (acknowledged)
- `namcap`, `lintian`, `rpmlint` were not run end-to-end because
  `unattended-upgrade` has been holding the dpkg lock at 99% CPU
  for >48 hours (a recurring issue per memory
  `unattended-upgrade-stuck-kill-public-publish-2026-08-06`).
  Manual sanity checks (`bash -n`, `make -n`, `rpmspec -P`)
  substituted. The linter gate will run in CI on the first
  clean build host.

### Anti-features (locked — unchanged from v2.0.0)
- No telemetry, no phoning home, no automatic update notifier.
- No server-authoritative mode.
- No AI-without-cost-gate.
- No Docker-only deploy.
- No closed-source build.
- No packaging-time shell-completion install — completions are
  generated on demand by the binary the operator just installed,
  not baked into a package.

## [2.1.0] — 2026-08-15

Minor release — fuzzy filter, sort modes, shell completions,
packaging templates, README refresh.

### Added
- **TUI fuzzy filter** — `crates/linkmarks-tui/src/filter.rs`
  (~190 LOC) wraps `nucleo = "0.5"` so the rest of the crate does
  not depend on its API surface. Press `/` to switch the filter
  from substring to fuzzy mode. `CaseMatching::Ignore` is the
  canonical mode for bookmark search (users typing `HELLO` almost
  always mean `hello`).
- **TUI sort modes** — `crates/linkmarks-tui/src/sort.rs`
  (~180 LOC): `SortMode::{CanonicalUrl, Title, CreatedDesc,
  UpdatedDesc}`. Stable tie-break preserves the input order so
  alternating sort modes does not visually jitter the list.
- **Shell completions** — new `completions <shell>` subcommand
  (`crates/linkmarks-cli/src/cmd/completions.rs`) emits a fresh
  completion script for `bash`, `zsh`, `fish`, `powershell`, and
  `elvish`. The script is regenerated from the live `Cli` parser
  at install time so a renamed flag surfaces immediately. 6
  tests cover each shell format (one structural signature +
  one that confirms every shell appears in `--help`).
- **Packaging templates** — declarative stubs under
  `debian/{control,rules,changelog,copyright,compat,source/format}`,
  `rpm/linkmarks.spec`, `arch/{PKGBUILD,.SRCINFO,linkmarks.install}`,
  `homebrew/linkmarks.rb`. Each directory has a `README.md`
  documenting the maintenance contract and the anti-feature
  conformance check. **No packages are built upstream**;
  downstream maintainers adapt the templates to their distro
  policy.
- **README refresh** — Install / Subcommands / CI / Exit codes /
  Project layout / Packaging / Environment sections all sync
  to v2.1.0.

### Changed
- Workspace version bump: 2.0.1 → **2.1.0**. Single source of
  truth in `[workspace.package] version`; all 6 crates inherit.
- `clap_complete = "4"` added to `[workspace.dependencies]`.

### Test surface
- Total tests: **286 / 286 green**
  (`cargo test --workspace --no-fail-fast`).
- `cargo clippy --workspace --all-targets -- -D warnings` clean.
- `cargo build --workspace --all-targets` clean.
- `cargo fmt --all -- --check` clean.

### Anti-features (locked — unchanged from v2.0.0)
- No telemetry, no phoning home, no automatic update notifier.
- No server-authoritative mode.
- No AI-without-cost-gate.
- No Docker-only deploy.
- No closed-source build.
- No packaging-time shell-completion install — completions are
  generated on demand by the binary the operator just installed,
  not baked into a package.

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

Interactive TUI + Firefox import + SQLite store.

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
  v2.0 introduces a new persistence surface (SQLite store, was
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

### Deferred
- **Fuzzy filter via `nucleo`** — implementation present in working tree
  (`crates/linkmarks-tui/src/filter.rs`, sort.rs, filter_fuzzy_test.rs) on
  branch `chore/snapshot-2026-08-15-fuzz-foreign-wt` (0f5b6cc). Tracked
  for the v2.1.2 PR alongside sort modes.
- **Sort modes** — sort.rs preserved on the snapshot branch.
- **Shell completions** — `clap_complete` + build script. Out of scope
  for this release.
- **README Install / Subcommands / CI sections refresh** — original v1
  install instructions remain. Tracked for a separate docs pass.
- **Debian / RPM / Arch / Homebrew packaging** — out of scope, follows
  in a future maintenance release once install.sh stabilizes.
- **`cargo fmt --check` 56 hunks of pre-existing drift** — to be batched
  with the next minor release.

### Planned
- Optional relay: CRDT sync server (`yrs` vs `automerge-rs` first),
  opt-in `linkmarks sync --server <url>`.
- Optional desktop GUI (Dioxus).
- Plugin market with stable ABI + signed binaries.

## [1.0.1] — 2026-08-03

### Changed
- **Tracking-param blocklist expanded** — added `mc_eid`, `mc_cid`,
  `igshid`, `ref_src` to the canonical-URL stripping set (was 5 → 9
  params).
- **SQLite WAL storage** — `linkmarks-core` storage layer now uses
  WAL journal mode + `busy_timeout=5000` for concurrent reader/writer
  safety. Migration is idempotent (`PRAGMA journal_mode=WAL` re-issued
  on every connection).
- **3 ADRs added** — Dioxus GUI choice, SQLite-as-store, and the
  tracking-param blocklist rationale.

## [1.0.0] — 2026-07-28

### Added
- Workspace skeleton: `linkmarks-core`, `linkmarks-cli`, `linkmarks-bridge-chromium`.
- `linkmarks-core` with normalized `Bookmark`, `SourceRef`, `Collection`,
  `Tag` model and `BookmarkSource` / `BookmarkSink` traits.
- URL canonicalization rules (with tracking-param blocklist
  `utm_*`, `fbclid`, `gclid`, `ref`, `ref_src`, `mc_eid`, `mc_cid`).
- Local deterministic dedupe by canonical URL with conflict report.
- `linkmarks-cli` with `list`, `import`, `export`, `dedupe` subcommands
  and `table` / `json` / `yaml` output formats.
- `linkmarks-bridge-chromium` parser for Chromium-family `Bookmarks`
  JSON (Chrome, Brave, Edge, Arc, Vivaldi, Opera).
- Fixture corpus: 5-bookmark anonymized example at
  `crates/bridges/linkmarks-bridge-chromium/tests/fixtures/chrome-bookmarks.example.json`.
- Licensing decision: AGPLv3 + Commercial dual.
- Exit codes: 0 OK, 1 partial, 2 invalid args, 3 dedupe conflicts.

### Anti-features (locked)
- No telemetry, no phoning home, no automatic update notifier.
- No server-authoritative mode.
- No AI-without-cost-gate.
- No Docker-only deploy.
- No closed-source build.

### Planned
- `linkmarks-tui` (ratatui), `linkmarks-bridge-firefox`, multi-source
  import.
- CRDT sync server (`yrs` vs `automerge-rs` spike first),
  `linkmarks sync --server <url>`.
- Desktop GUI (Dioxus).
- Plugin market with stable ABI and signed binaries.
