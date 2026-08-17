# linkmarks

> Local-first, AGPL-licensed bookmark manager. Imports what you have,
> dedupes deterministically by canonical URL, stays useful offline
> against a single SQLite store under `~/.local/share/linkmarks/`.

`linkmarks` is the umbrella crate for the [LinkMarks](https://github.com/LOUST-PRO/LinkMarks)
workspace. It bundles the seven sub-libraries — the core domain model,
the CLI dispatcher, the terminal UI, and three browser-bridge parsers
— into one artifact so end users only need a single `cargo install` to
get the working CLI.

## What is it

LinkMarks is a bookmark manager that lives entirely on your disk. It
imports from the browser export formats you already have (Chrome/Edge
JSON, Firefox `places.sqlite`, Netscape HTML, jsonlz4 backups),
normalizes URLs to a canonical form, deduplicates deterministically,
and lets you browse the result in a terminal UI. No account, no
remote, no telemetry. The SQLite store at `~/.local/share/linkmarks/`
is the single source of truth.

The umbrella collapses the seven previously-individual crates into one
artifact. Sub-libraries remain installable via the umbrella's `path`
dependencies but no longer ship their own crates.io pages, so the
project presents as the single tool users actually see.

## Install

Install the CLI binary from crates.io:

```bash
cargo install linkmarks --locked
```

This produces the `linkmarks` binary in `~/.cargo/bin/`. The binary
embeds the full CLI dispatcher (from `linkmarks-cli`) plus the
terminal UI (from `linkmarks-tui`) and all three browser bridges
(chromium, firefox, netscape).

If you want to use LinkMarks as a Rust library — to embed the
canonicalization, the storage layer, or the TUI into your own crate
— add it to `Cargo.toml`:

```toml
[dependencies]
linkmarks = "2.2"
```

The umbrella re-exports `linkmarks_core` (domain model, storage,
canonicalization, dedupe) and `linkmarks_tui` (interactive terminal
browser) so consumers can `use linkmarks::{core, tui}` directly.

## Usage

Initialize the local store and import from any of the supported
formats:

```bash
linkmarks init
linkmarks import --from chrome ./Chrome-Bookmarks.html
linkmarks import --from firefox ~/snap/firefox/common/.mozilla/firefox/*/places.sqlite
linkmarks list
```

Launch the interactive terminal browser to skim and search:

```bash
linkmarks tui
```

Deduplicate deterministically (canonical URL + title + tag overlap)
with a conflict report:

```bash
linkmarks dedupe --report
```

Export to any supported format:

```bash
linkmarks export --to netscape ./my-bookmarks.html
linkmarks export --to json ./my-bookmarks.json --format json
```

Generate shell completions:

```bash
linkmarks completions bash > ~/.local/share/bash-completion/completions/linkmarks
linkmarks completions zsh  > "${fpath[1]}/_linkmarks"
linkmarks completions fish > ~/.config/fish/completions/linkmarks.fish
```

Every subcommand supports `--format json|yaml` for machine-readable
output.

## Architecture

The umbrella re-exports:

- `linkmarks_core` — domain model (`Bookmark`, `Tag`, `Collection`),
  SQLite storage layer, URL canonicalization, deterministic dedupe.
- `linkmarks_tui` — interactive terminal browser (ratatui +
  crossterm), with state-machine integration tests for every filter
  and sort variant.
- `linkmarks_cli` — clap-based CLI dispatcher. The umbrella binary
  delegates its `main` to `linkmarks_cli::run()`, so the CLI is
  single-sourced.
- `linkmarks_bridge_chromium` / `_firefox` / `_netscape` — the three
  interchange parsers. Browser-locked formats and standard HTML
  interchange both supported.

The seven sub-libraries are workspace members with `publish = false`
— they live in this monorepo and are bundled into the umbrella at
build time. There are no separate crates.io pages for the sub-libraries.

## License

Dual-licensed under **AGPL-3.0-or-later** (open source) or a
commercial license. See [`LICENSE`](https://github.com/LOUST-PRO/LinkMarks/blob/main/LICENSE)
at the repository root for the full text and the commercial contact
path.

SPDX: `AGPL-3.0-or-later OR LicenseRef-Commercial`
