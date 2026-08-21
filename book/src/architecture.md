# Architecture

The LinkMarks workspace is 8 Cargo crates organised in 4
layers: core library, bridges, TUI, CLI/umbrella. This page
documents the dependency graph, the layer boundaries, and the
where-to-find-what map for contributors.

## Workspace layout

```text
LinkMarks/
├── Cargo.toml                # Workspace root
├── Cargo.lock                # Shared lockfile
├── crates/
│   ├── linkmarks/           # Umbrella binary (re-exports CLI)
│   ├── linkmarks-cli/       # Command dispatch
│   ├── linkmarks-core/      # Library: SQLite, dedup, sort, filter, sync
│   ├── linkmarks-tui/       # Interactive ratatui browser
│   ├── linkmarks-bridge-chromium/
│   ├── linkmarks-bridge-firefox/
│   ├── linkmarks-bridge-netscape/
│   └── linkmarks-bench-crdt/  # Private benchmark harness
├── docs/
│   ├── man/linkmarks.1       # Canonical man page
│   └── relay-deployment.md   # Self-hosted relay guide (preview)
├── book/                    # This mdbook site
├── arch/PKGBUILD
├── debian/
├── rpm/
└── homebrew/Formula/
```

## Dependency graph

```text
                       ┌────────────────────┐
                       │     linkmarks      │  (umbrella binary)
                       └─────────┬──────────┘
                                 │
                                 ▼
                       ┌────────────────────┐
                       │   linkmarks-cli    │
                       └─────────┬──────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
              ▼                  ▼                  ▼
      ┌─────────────┐    ┌──────────────┐    ┌─────────────┐
      │ linkmarks-  │    │ linkmarks-   │    │ linkmarks-  │
      │   core      │    │   tui        │    │   bridges   │
      └─────────────┘    └──────┬───────┘    └──────┬──────┘
                                │                   │
                                ▼                   ▼
                       ┌─────────────┐    ┌──────────────────────┐
                       │ linkmarks-  │    │ linkmarks-bridge-*   │
                       │   core      │    │ (chromium/firefox/   │
                       │             │    │  netscape)           │
                       └─────────────┘    └──────────────────────┘
```

The bridges depend on `linkmarks-core` for the canonical-URL
helper. The TUI depends on `linkmarks-core` for the SQLite
reader. The CLI depends on all of them.

## Layer responsibilities

### `linkmarks-core` (~3,500 LOC)

The pure-library layer. Has zero network I/O, zero CLI
integration, zero TUI integration. Public API surface:

| Module | Purpose |
|---|---|
| `canonical` | Canonical URL computation |
| `schema` | SQLite schema + migrations |
| `store` | CRUD over the SQLite store |
| `dedupe` | Deduplication pass |
| `sort` | Sort mode enum + comparators |
| `filter` | Filter mode enum + matchers |
| `bridge` | Bridge trait + shared types |
| `sync` | yrs sub-document merge |

### `linkmarks-bridge-*` (~400 LOC each)

Format-specific parsers. Each bridge:

1. Implements the `Bridge` trait from `linkmarks-core`.
2. Has its own dependency tree (e.g. `linkmarks-bridge-firefox`
   uses `rusqlite` to read `places.sqlite`; the others don't).
3. Has its own fixture under `tests/fixtures/`.
4. Has a round-trip test that imports → exports → re-imports.

The three bridges share no code beyond the `Bridge` trait.

### `linkmarks-tui` (~2,200 LOC)

The interactive ratatui browser. Depends on `linkmarks-core`
for the read-only store, on `nucleo` for fuzzy matching, and on
`crossterm` for terminal I/O.

Public API surface:

| Module | Purpose |
|---|---|
| `app` | The `App` struct (state machine) |
| `input` | Key event → action dispatcher |
| `render` | ratatui render loop |
| `theme` | Color theme registry |

The TUI does not depend on `linkmarks-cli`. It is a separate
binary that ships alongside the CLI.

### `linkmarks-cli` (~1,800 LOC)

The clap-based command dispatcher. Public API surface:

| Module | Purpose |
|---|---|
| `main` | clap parser + dispatch |
| `cmd` | Per-subcommand implementations |
| `config` | Config file loader |
| `output` | Output formatting (table, json, csv) |

The CLI is the only crate that wires `linkmarks-core`,
`linkmarks-tui`, and the bridges together.

### `linkmarks` (umbrella, ~50 LOC)

The single-binary umbrella that re-exports the CLI's `main`:

```rust
fn main() {
    linkmarks_cli::run();
}
```

The umbrella exists so users can `cargo install linkmarks` and
get a single canonical binary, while library users can depend
on `linkmarks-core` directly.

### `linkmarks-bench-crdt` (private)

Benchmark harness for the yrs CRDT merge path. NOT published to
crates.io. Not in the umbrella binary. The CI runs the benchmarks
nightly and posts results to a private dashboard.

## Public API contracts

Every public crate ships an `API.toml` (a small TOML manifest)
documenting the public surface. The CI enforces that no
`pub` symbol is added without an entry in `API.toml`.

This is the guarantee that lets LinkMarks ship semver-meaningful
minor releases without breaking downstream library users.

## Where to find what

| If you want to... | Look at... |
|---|---|
| Add a new sort mode | `linkmarks-core/src/sort.rs` |
| Add a new filter mode | `linkmarks-core/src/filter.rs` |
| Add a new bridge | copy `linkmarks-bridge-netscape/`, edit the parser |
| Add a new CLI subcommand | `linkmarks-cli/src/cmd/` |
| Add a new TUI keybinding | `linkmarks-tui/src/input.rs` |
| Add a new SQLite column | `linkmarks-core/src/schema.rs` + write a migration |
| Change the canonical URL rules | `linkmarks-core/src/canonical.rs` |
| Change the yrs merge semantics | `linkmarks-core/src/sync/` |
| Add a new export format | `linkmarks-cli/src/cmd/export.rs` |
| Add a new theme | `linkmarks-tui/src/theme/` |

## Build flags

| Feature | Purpose |
|---|---|
| `default` | Stable features |
| `sync` | yrs CRDT sync layer |
| `bench` | Benchmark harness (private only) |
| `static-sqlite` | Bundled SQLite (no system dep) |

The default feature set is `["sync"]`. Disabling `sync` removes
the `linkmarks sync` subcommand and the yrs dependency from the
core library.