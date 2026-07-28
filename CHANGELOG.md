# Changelog — LinkMarks

All notable changes to this project are documented here. The format
follows [Keep a Changelog](https://keepachangelog.com/) and the
project adheres to [Semantic Versioning](https://semver.org/).

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
