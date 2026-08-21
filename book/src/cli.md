# CLI reference

The `linkmarks` binary exposes 13 subcommands. Every subcommand
supports `--help`, accepts `-h` as shorthand, and exits 0 on
success, 1 on user error, 2 on store error, 3 on import/parse
error.

## Global flags

```text
linkmarks [OPTIONS] <COMMAND>

Options:
  -C, --config <PATH>    Override config file path (default: XDG)
  -v, --verbose          Increase log verbosity (-v, -vv, -vvv)
  -q, --quiet            Suppress non-error output
  -V, --version          Print version and exit
  -h, --help             Print help and exit
```

## Subcommands

### `init`

Initialise the store and config file.

```text
Usage: linkmarks init [OPTIONS]

Options:
      --force            Reinitialise even if store exists
      --store-dir <PATH> Override the XDG store directory
```

### `import`

Import bookmarks from a browser export.

```text
Usage: linkmarks import <FORMAT> <SOURCE>

Arguments:
  <FORMAT>   One of: chromium, firefox, netscape
  <SOURCE>   Path to the export (file or directory depending on format)

Options:
      --dry-run          Show what would be imported without writing
      --limit <N>        Cap the number of records imported
      --with-annotations  Firefox only: include moz_annos
```

Examples:

```bash
linkmarks import chromium ~/.config/chromium/Default/Bookmarks
linkmarks import firefox ~/.mozilla/firefox/abc.default/places.sqlite --with-annotations
linkmarks import netscape ~/Downloads/pocket-export.html --dry-run
```

### `list`

List bookmarks.

```text
Usage: linkmarks list [OPTIONS]

Options:
      --limit <N>          Cap the number of records shown
      --sort <MODE>        One of: updated, title, canonical-url, created
      --filter <MODE>      One of: substring, tag, fuzzy
      --query <Q>          Pre-populate the filter query
      --format <FMT>       One of: table, json, csv, plain
      --tag <TAG>          Filter to records carrying <TAG> (repeatable)
      --folder <PATH>      Filter to records in <PATH> (repeatable)
      --no-header          Omit header row (table, csv formats)
```

Examples:

```bash
linkmarks list --limit 10
linkmarks list --tag rust --format json
linkmarks list --query ratatui --filter fuzzy
linkmarks list --folder "Tech/Rust" --folder "Tech/Go"
```

### `add`

Add a single bookmark.

```text
Usage: linkmarks add <URL> [OPTIONS]

Arguments:
  <URL>     The URL to bookmark

Options:
  -t, --title <TITLE>    Bookmark title (defaults to <URL>)
  -T, --tag <TAG>        Tag (repeatable)
  -n, --notes <NOTES>    Free-form notes
      --folder <PATH>    Place in folder hierarchy
```

### `delete`

Delete a bookmark by ID or canonical URL.

```text
Usage: linkmarks delete <TARGET>

Arguments:
  <TARGET>    ULID or canonical URL

Options:
      --dry-run    Show what would be deleted
      --force      Skip confirmation prompt
```

### `update`

Update bookmark fields.

```text
Usage: linkmarks update <TARGET> [OPTIONS]

Arguments:
  <TARGET>    ULID or canonical URL

Options:
      --title <TITLE>    Set title
      --notes <NOTES>    Set notes
      --add-tag <TAG>    Add tag (repeatable)
      --rm-tag <TAG>     Remove tag (repeatable)
      --folder <PATH>    Set folder
```

### `dedupe`

Run the canonical-URL dedupe pass.

```text
Usage: linkmarks dedupe [OPTIONS]

Options:
      --dry-run         Show the dedupe plan without executing
      --limit <N>       Cap the number of groups processed
      --verbose         Print per-group winners and losers
```

### `export`

Export the store to another format.

```text
Usage: linkmarks export <FORMAT> [OPTIONS]

Arguments:
  <FORMAT>    One of: netscape, json, csv

Options:
  -o, --output <PATH>    Output path (default: stdout)
      --limit <N>        Cap records exported
      --query <Q>        Filter query
```

### `sync`

Push or pull from the self-hosted relay (preview).

```text
Usage: linkmarks sync <DIRECTION> [OPTIONS]

Arguments:
  <DIRECTION>    One of: push, pull, status

Options:
      --remote <URL>     Override the relay URL from config
      --dry-run          Show what would be sent/received
      --limit <N>        Cap the number of records
      --since <RFC3339>  Only push/pull records updated since
```

### `tui`

Launch the interactive TUI browser.

```text
Usage: linkmarks tui [OPTIONS]

Options:
      --sort <MODE>      Initial sort mode
      --filter <MODE>    Initial filter mode
      --query <Q>        Pre-populate filter query
      --theme <NAME>     Color theme: rust, light, dark, ayu
```

### `completions`

Generate shell completions.

```text
Usage: linkmarks completions <SHELL>

Arguments:
  <SHELL>    One of: bash, zsh, fish, elvish, powershell

Options:
  -o, --output <PATH>    Output path (default: stdout)
```

### `doctor`

Diagnose the install.

```text
Usage: linkmarks doctor

Options:
      --check <NAME>     Run a single check (one of: store, config, indices, sync)

Checks:
  store      Verifies the SQLite file exists and is readable
  config     Validates config.toml against the schema
  indices    Confirms unique indexes on canonical_url exist
  sync       Tests connectivity to the configured relay
```

### `version`

Print version, build profile, and feature flags.

```text
Usage: linkmarks version

Output:
  linkmarks 2.2.0
  profile: release
  features: default
  sqlite: 3.46.1
  yrs: 0.18.5
  cargo: 1.84.0
  rustc: 1.84.0 (edition 2021)
```

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | User error (bad flags, missing arguments) |
| 2 | Store error (SQLite open, schema mismatch) |
| 3 | Import/parse error (malformed input) |
| 4 | Sync error (relay unreachable, conflict) |
| 5 | Permission error (cannot read source, cannot write store) |
| 64 | Configuration error (config.toml invalid) |

The exit codes are stable across releases; downstream tooling can
rely on them.