# Reference

This page is the technical reference: env vars, file layout,
config schema, exit codes, signal handling.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `LINKMARKS_CONFIG` | `$XDG_CONFIG_HOME/linkmarks/config.toml` | Path to the config file |
| `LINKMARKS_STORE` | `$XDG_DATA_HOME/linkmarks/linkmarks.db` | Path to the SQLite store |
| `LINKMARKS_RELAY` | from config | Override the relay URL |
| `LINKMARKS_TOKEN` | from config | Bearer token for the relay |
| `LINKMARKS_THEME` | `rust` | Default TUI theme |
| `LINKMARKS_NO_SYNC` | unset | Set to `1` to disable `linkmarks sync` |
| `RUST_LOG` | `info` | Standard `env_logger` filter |
| `NO_COLOR` | unset | If set, TUI disables ANSI colors |

The `XDG_*` defaults follow the XDG Base Directory
Specification.

## File layout

```text
$XDG_CONFIG_HOME/linkmarks/
├── config.toml             # Main config
├── keymap.toml             # TUI keymap overrides (optional)
└── relay.toml              # Relay credentials (optional)

$XDG_DATA_HOME/linkmarks/
├── linkmarks.db            # SQLite store (single file)
├── linkmarks.db-wal        # Write-Ahead Log (SQLite-managed)
├── linkmarks.db-shm        # Shared memory (SQLite-managed)
└── sync/                   # yrs sub-document snapshots (preview)

$XDG_CACHE_HOME/linkmarks/
└── nucleo/                 # In-memory matcher cache (regenerated each session)
```

## Config schema

The full schema lives in `linkmarks-cli/src/config.rs`. The
`config.toml.example` file in the repo root shows every field
with comments.

```toml
# ~/.config/linkmarks/config.toml

# SQLite store path (default: $XDG_DATA_HOME/linkmarks/linkmarks.db)
store = "/var/lib/linkmarks/linkmarks.db"

# Default sort mode for `linkmarks list` and the TUI
# One of: updated, title, canonical-url, created
default_sort = "updated"

# Default filter mode for the TUI
# One of: substring, tag, fuzzy
default_filter = "substring"

# Sync relay URL (used by `linkmarks sync`)
relay = "https://relay.example.com"

# Sync relay bearer token (read from $LINKMARKS_TOKEN if not set)
# Prefer the env var to avoid storing secrets on disk
token = "${LINKMARKS_TOKEN}"

# TUI theme (one of: rust, light, dark, ayu)
theme = "rust"

# Folder depth limit (default: 8)
max_folder_depth = 8

# Whether to keep separators from browser imports (default: false)
keep_separators = false

# Bridge-specific overrides
[bridges.chromium]
# Override the default ULID prefix for Chromium-imported bookmarks
# (default: "chr")
ulid_prefix = "chr"

[bridges.firefox]
# Include moz_annos by default (default: false)
with_annotations = false

[bridges.netscape]
# Preserve <DD> comments as notes (default: false)
dd_as_notes = false
```

## Exit codes

| Code | Name | Description |
|---|---|---|
| 0 | `Success` | Command succeeded |
| 1 | `UserError` | Invalid flags, missing arguments |
| 2 | `StoreError` | SQLite open failed, schema mismatch |
| 3 | `ParseError` | Bridge parser could not parse the input |
| 4 | `SyncError` | Relay unreachable, merge conflict |
| 5 | `PermissionError` | Cannot read source, cannot write store |
| 64 | `ConfigError` | Config file invalid |
| 66 | `NoInput` | Expected a TTY, got a pipe |
| 73 | `CantCreate` | Cannot create a file or directory |
| 130 | `Interrupted` | SIGINT received (Ctrl+C) |

The codes are stable across releases.

## Signals

The CLI honours three signals:

| Signal | Effect |
|---|---|
| `SIGINT` (Ctrl+C) | Graceful shutdown. The SQLite WAL is flushed and the connection is closed. Exit code 130. |
| `SIGTERM` | Same as SIGINT. Used by systemd `Type=oneshot` services. |
| `SIGHUP` | Config reload. Re-reads `config.toml` without restart. |

The TUI additionally honours `SIGWINCH` (terminal resize) for
re-rendering.

## File lock

A single advisory file lock at `$XDG_DATA_HOME/linkmarks/.lock`
prevents two CLI instances from racing on the same store. The
lock is released on graceful shutdown.

## Schema migrations

The store has a `schema_version` table with a single row. Each
migration is a `linkmarks-core/src/migrations/NNNN_description.sql`
file. The migrations run automatically on `linkmarks init` and
on every CLI invocation (in a single transaction).

The current schema version is `7`.

## Performance budgets

| Operation | Target | Tested |
|---|---|---|
| `linkmarks init` | < 100 ms | 12 ms (cold cache, ext4) |
| `linkmarks import chromium` (1000 records) | < 5 s | 1.4 s |
| `linkmarks dedupe` (10,000 records) | < 5 s | 1.9 s |
| `linkmarks list --limit 100` | < 50 ms | 8 ms |
| `linkmarks tui` startup | < 200 ms | 90 ms |
| `linkmarks sync push` (1000 changed records) | < 3 s | 0.7 s |
| `linkmarks sync pull` (1000 changed records) | < 3 s | 0.9 s |

These budgets are enforced by the benchmark suite in
`linkmarks-bench-crdt`.

## Debugging

```bash
# Increase verbosity (can be repeated)
linkmarks -vv list

# Full debug logging to stderr
RUST_LOG=linkmarks_core=debug,linkmarks_cli=debug linkmarks list

# Profile a slow query
RUST_LOG=linkmarks_core::store=trace linkmarks dedupe
```

The TUI logs to a per-session file at
`$XDG_CACHE_HOME/linkmarks/tui-YYYY-MM-DD-HHMMSS.log` (if the
`--log-file` flag is passed).