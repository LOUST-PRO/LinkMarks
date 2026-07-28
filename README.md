# LinkMarks

![License](https://img.shields.io/badge/license-AGPL--3.0%2Bcommercial-blue)
![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange)
![Version](https://img.shields.io/badge/version-v1.0.0-blue)
![Self-hosted](https://img.shields.io/badge/self--hosted-yes-green)
![Local-first](https://img.shields.io/badge/local--first-yes-green)
![No telemetry](https://img.shields.io/badge/telemetry-no-red)
![Single binary](https://img.shields.io/badge/distribution-single%20binary-orange)
![CRDT sync](https://img.shields.io/badge/sync-CRDT-yellow)
![No Docker required](https://img.shields.io/badge/docker-optional-lightgrey)

A local-first, AGPL-licensed bookmark manager. Imports what you already have,
dedupes deterministically, and stays useful offline. The server is an optional
relay, never the authority.

Topics: `bookmarks` `self-hosted` `rust` `local-first` `crdt` `bookmark-manager` `agpl` `cli` `linkmarks`

---

## What it does (v1)

LinkMarks is a single Rust binary that:

1. Imports Chromium-style `Bookmarks` JSON (`google-chrome`, `Brave`,
   `Edge`, `Arc`, `Vivaldi`, `Opera`).
2. Lists bookmarks deterministically (`table` / `json` / `yaml`).
3. Exports to Netscape HTML (the universal interchange format).
4. Dedupes locally by canonical URL with a human-readable conflict
   report.

That's it for v1. Everything else is in the [ROADMAP](./docs/ROADMAP.md)
and is opt-in, not required.

## Anti-features

These are decisions, not gaps. Adding them later requires a CONCERNS.md
entry.

- **No server-authoritative mode.** The server is relay-only.
- **No mandatory telemetry.** No phoning home. Ever.
- **No silent link-health checks.** We never visit your URLs to "see if
  they're alive". That leaks intent and costs you money.
- **No AI-without-cost-gate.** No embeddings, no LLM-suggested tags,
  no auto-summary. If these ever ship, they must declare per-action
  cost before running.
- **No Docker-only deploy.** Single static binary, systemd-friendly.
- **No closed-source build.** Source == release.

## Install

Pre-built binaries will land on the GitHub Releases page once v1 ships.
Until then:

```bash
git clone https://github.com/LOUST-PRO/LinkMarks
cd LinkMarks
cargo build --release
./target/release/linkmarks --help
```

## Usage

```bash
# Import a Chromium Bookmarks file
linkmarks import --source=chrome --path ~/.config/google-chrome/Default/Bookmarks

# List deterministically
linkmarks list --source=chrome --format=table

# Export to Netscape HTML
linkmarks export --format=netscape --output ./bookmarks.html

# Dedupe with a conflict report (dry-run by default)
linkmarks dedupe --source=chrome
linkmarks dedupe --source=chrome --apply   # explicit apply token
```

Exit codes:

| Code | Meaning |
|---|---|
| 0 | OK |
| 1 | Partial / source error |
| 2 | Invalid args |
| 3 | Dedupe conflicts found |

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md`. Contact: `opensource@loust.pro`.

## Architecture

See [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) for the workspace
tree, crate responsibilities, and the domain model. Decisions live
under [`docs/decisions/`](./docs/decisions/) as ADRs (MADR template).

## Contributing

Issues and PRs welcome. Read [`CONTRIBUTING`](./.github/PULL_REQUEST_TEMPLATE.md)
and the [CONCERNS](./docs/CONCERNS.md) file before opening a PR that
adds scope.

## Maintainer

David Alejandro Mireles Llamas — [@loust](https://github.com/loust)
