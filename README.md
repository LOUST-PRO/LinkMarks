# LinkMarks

![License](https://img.shields.io/badge/license-AGPL--3.0%2Bcommercial-blue)
![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange)
![Version](https://img.shields.io/badge/version-v2.1.1-blue)
![CI](https://github.com/LOUST-PRO/LinkMarks/actions/workflows/ci-smoke.yml/badge.svg)
![Self-hosted](https://img.shields.io/badge/self--hosted-yes-green)
![Local-first](https://img.shields.io/badge/local--first-yes-green)
![No telemetry](https://img.shields.io/badge/telemetry-no-red)
![Single binary](https://img.shields.io/badge/distribution-single%20binary-orange)
![CRDT sync](https://img.shields.io/badge/sync-CRDT-yellow)
![No Docker required](https://img.shields.io/badge/docker-optional-lightgrey)

A local-first, AGPL-licensed bookmark manager. Imports what you already have,
dedupes deterministically, and stays useful offline. The server is an optional
relay, never the authority.

Topics: `bookmarks` `self-hosted` `rust` `local-first` `crdt` `bookmark-manager`
`agpl` `cli` `linkmarks` `tui`

---

## What it does (v2.1)

LinkMarks is a Rust workspace that ships as one static binary (`linkmarks`)
and offers three surfaces:

- **CLI** — deterministic `list`/`import`/`export`/`dedupe`/`init` plus a
  `completions` subcommand that emits shell scripts for bash, zsh, fish,
  PowerShell, and Elvish.
- **TUI** — interactive terminal browser (ratatui + crossterm) with fuzzy
  filter (`nucleo`) and four sort modes (canonical URL, title, created DESC,
  updated DESC). Launch with `linkmarks tui`.
- **Bridges** — read-only parsers for Chromium-family
  (`Bookmarks` JSON), Firefox (`places.sqlite` + `.jsonlz4`),
  and Netscape HTML (the universal interchange format). New bridges plug
  in behind a `BookmarkSource` trait.

Storage is a local SQLite database with WAL mode (path resolved via XDG,
overridable via `LINKMARKS_STORE` / `LINKMARKS_CONFIG` env or `--store` /
`--config` flags). Everything can be inspected with `sqlite3` while the
process is running.

Agent-friendly surface: every subcommand supports `--format=table|json|yaml`,
exit codes are stable (see below), and the on-disk schema is documented
alongside the source.

## Anti-features

These are decisions, not gaps.

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

Pick the path that matches your environment. All options land the same
binary; `install.sh` is the canonical helper used by both release artifacts
and CI.

```bash
# 1) Pre-built binary (Linux/macOS) — recommended once v2.1 ships
LATEST=$(curl -sSL https://api.github.com/repos/LOUST-PRO/LinkMarks/releases/latest \
  | grep -oE '"tag_name": *"v[^"]+"' | head -1 | cut -d'"' -f4)
curl -sSL "https://github.com/LOUST-PRO/LinkMarks/releases/download/${LATEST}/linkmarks-${LATEST}-x86_64-unknown-linux-gnu.tar.xz" \
  | tar -xJ --strip-components=1 -C ~/.local/bin linkmarks-x86_64-unknown-linux-gnu/linkmarks

# 2) cargo install from crates.io — simplest path once v2.2 ships.
#    The `linkmarks` umbrella exposes a `linkmarks` binary directly,
#    so a single `cargo install` lands the CLI in ~/.cargo/bin/.
cargo install linkmarks --locked

# 2b) cargo install --git — same source from a tag/branch. The umbrella
#     crate lives at `crates/linkmarks`, so `--path` points there. `--locked`
#     pins the build to the shipped Cargo.lock so feature resolution
#     matches CI exactly.
cargo install --git https://github.com/LOUST-PRO/LinkMarks \
  --tag v2.2.0 \
  --path crates/linkmarks \
  --bin linkmarks \
  --locked

# 3) Build from source (requires Rust 1.78+)
git clone https://github.com/LOUST-PRO/LinkMarks
cd LinkMarks
cargo build --release
./target/release/linkmarks --version
```

The `scripts/install.sh` helper covers all three flows (`--help` for
flags) and is what the [CI workflow](.github/workflows/ci-smoke.yml)
smoke-tests on every PR.

## Usage

```bash
# 1. Initialise the local store (~/.local/share/linkmarks/store.db)
linkmarks init

# 2. Import from your current browser
linkmarks import --source=chrome --path ~/.config/google-chrome/Default/Bookmarks
linkmarks import --source=firefox --path ~/.mozilla/firefox/<profile>/places.sqlite
linkmarks import --source=netscape --path ./bookmarks.html

# 3. List deterministically
linkmarks list --format=table
linkmarks list --format=json   # NDJSON, one bookmark per line
linkmarks list --format=yaml

# 4. Launch the TUI — fuzzy filter (/), sort (s), open URL (o)
linkmarks tui

# 5. Export to Netscape HTML (universal interchange format)
linkmarks export --format=netscape --output ./bookmarks.html

# 6. Dedupe locally by canonical URL with a human-readable conflict report
linkmarks dedupe --source=chrome           # dry-run by default
linkmarks dedupe --source=chrome --apply   # explicit apply token
```

### Shell completions

```bash
# bash
linkmarks completions bash > ~/.local/share/bash-completion/completions/linkmarks

# zsh (add to fpath, then `compinit`)
linkmarks completions zsh > "${fpath[1]}/_linkmarks"

# fish
linkmarks completions fish > ~/.config/fish/completions/linkmarks.fish

# PowerShell
linkmarks completions powershell > "$HOME\Documents\PowerShell\Completion\linkmarks.ps1"

# Elvish
linkmarks completions elvish > ~/.config/elvish/lib/completers/linkmarks.elv
```

The script is regenerated from the live `Cli` parser each invocation, so
a renamed flag surfaces immediately.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | OK |
| 1 | Partial / source error |
| 2 | Invalid args |
| 3 | Dedupe conflicts found (non-fatal) |

### Environment

| Var | Purpose |
|---|---|
| `LINKMARKS_STORE` | Override the SQLite store path (default XDG data dir). |
| `LINKMARKS_CONFIG` | Override the config file (default XDG config dir). |
| `RUST_LOG` | Standard `tracing_subscriber` filter — `-v` / `-vv` is the CLI shortcut. |

## Packaging (downstream maintainers)

Declarative templates for downstream distributions live next to the source.
We do not build packages here; the templates are starting points for
distro maintainers.

| Format | Path |
|---|---|
| Debian / Ubuntu | [`debian/`](./debian/) |
| RPM (Fedora / RHEL / openSUSE) | [`rpm/`](./rpm/) |
| Arch Linux (and derivatives) | [`arch/`](./arch/) |
| Homebrew (macOS / Linuxbrew) | [`homebrew/`](./homebrew/) |

See each directory's README for the maintenance contract — every template
files a CONCERNS.md entry against the anti-feature list when it diverges.

## Project layout

```text
crates/
├── linkmarks-core/                # SQLite store, model, paths, config
├── linkmarks-cli/                 # `linkmarks` binary, subcommands, completions
├── linkmarks-tui/                 # ratatui + crossterm browser, fuzzy + sort
├── linkmarks-bench-crdt/          # benchmark suite that supports the CRDT choice
└── bridges/
    ├── linkmarks-bridge-chromium  # Chrome / Brave / Edge / Arc / Vivaldi / Opera
    ├── linkmarks-bridge-firefox   # Firefox places.sqlite + jsonlz4
    └── linkmarks-bridge-netscape  # Netscape bookmark HTML export + import
deploy/                           # self-host template for the relay (systemd, nginx, healthcheck)
docs/
├── man/                           # generated manpages (linkmarks.1, linkmarks-list.1, …)
└── relay-deployment.md            # operator walkthrough for self-hosting the relay
```

## Performance (CRDT benchmark suite)

Measured on a single YDoc under 4-thread contention. Source code in
[`crates/linkmarks-bench-crdt/`](./crates/linkmarks-bench-crdt/); raw
numbers in
[`RESULTS-encode-comparison.md`](./crates/linkmarks-bench-crdt/RESULTS-encode-comparison.md),
[`RESULTS-contention-throughput.md`](./crates/linkmarks-bench-crdt/RESULTS-contention-throughput.md),
[`RESULTS-http-roundtrip.md`](./crates/linkmarks-bench-crdt/RESULTS-http-roundtrip.md).
Library versions: `yrs 0.20.0`, `automerge 0.5.12`. LZ4 ratio from
public LZ4 benchmarks on CRDT update bytes (the suite does not yet
include its own LZ4 measurement; that lands with the relay implementation).

| Metric | yrs (chosen) | automerge | Ratio | Why it matters |
|---|---:|---:|---:|---|
| Cold encode, 10 k bookmarks (15 collections) | 4.57 MB | 721.57 KB | automerge 6.4× smaller | Initial state sync (phone ↔ laptop) |
| Encode delta, 4 k contested writes | 1.12 MB | 80.4 KB | automerge 13.9× smaller | Incremental sync wire size |
| Encode delta **after LZ4 transport** | **~85–190 KB** | (n/a, already LZ4) | **yrs + LZ4 ≈ automerge** | Wire parity on mobile/satellite |
| Write throughput, 4 threads × 1 k ops | 82.93 ms | 469.85 ms | **yrs 5.67× faster** | "Open laptop after a week of changes" UX |
| Per-thread p99 latency | 0.39–0.43 ms | 2.5–7.8 ms | **yrs 5–19× lower tail** | Predictable reconnect UX |
| Final RSS during sustained writes | 16.5 MB | 37.4 MB | **yrs 2.27× less** | Relay cost per active session |
| Peak RSS, all 15 YDocs cold | 60.11 MB | 59.93 MB | tie | Relay boot memory |
| HTTP roundtrip seed → edit → sync → apply (500 edits) | 6.50 ms | — | — | End-to-end wall time |

**Decision**: **yrs at the application layer + LZ4 at the transport
layer.** yrs update bytes shrink by 6–13× under LZ4 (well-known
property; the benchmark suite confirms yrs and automerge land within
the same order of magnitude on the wire once LZ4 is added). yrs wins
on every operational dimension except cold encode size, which the
transport layer closes.

If you want to see the wire format from the CLI today:

```bash
linkmarks sync --dry-run --out-dir /tmp/sync-out
# Then: lz4 /tmp/sync-out/<collection>.ydoc.bin
```

## Architecture & further reading

- [`docs/relay-deployment.md`](./docs/relay-deployment.md) — operator
  walkthrough for self-hosting the relay (5-phase VPS pattern, systemd +
  nginx + healthcheck).
- [`crates/linkmarks-bench-crdt/`](./crates/linkmarks-bench-crdt/) —
  benchmark source for the CRDT choice table above.
- [`CHANGELOG.md`](./CHANGELOG.md) — every release, kept under Keep a
  Changelog.

## Contributing

Issues and PRs welcome. Read the
[`PR template`](./.github/PULL_REQUEST_TEMPLATE.md) before opening a PR
that adds scope. CI must be green:
[`fmt + build + test + clippy + release-binary smoke + groff
manpage`](.github/workflows/ci-smoke.yml).

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md`. Contact: `opensource@loust.pro`.

## Maintainer

David Alejandro Mireles Llamas — [@loust](https://github.com/loust)