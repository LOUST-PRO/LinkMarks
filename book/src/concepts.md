# Concepts

This page explains the four concepts the rest of LinkMarks builds
on: canonical URL, the SQLite schema, the dedup algorithm, and the
sort/filter modes. Understanding these makes the [CLI reference](./cli.md)
and the [TUI browser](./tui.md) feel like obvious consequences.

## Canonical URL

The single most important concept in LinkMarks is the **canonical
URL**: a normalised form of a URL where two URLs that "look the
same" to a human are byte-identical after canonicalisation.

A canonical URL is computed by applying these rules, in order:

1. Lowercase the scheme (`https://` stays `https://`).
2. Lowercase the host (`Example.COM` becomes `example.com`).
3. Remove the default port for the scheme (`:443` for `https`,
   `:80` for `http`).
4. Remove the fragment (`#section-2` is gone).
5. Remove trailing slash on the path (except for the empty path
   itself, which stays `/`).
6. Sort query parameters lexicographically by name, then by value.
7. Recode percent-encoding to NFC unicode normalisation.

After canonicalisation, the following URLs are all byte-identical:

```
https://Example.com/foo?b=2&a=1
https://example.com/foo?a=1&b=2
HTTPS://example.com:443/foo/?b=2&a=1#section-2
```

This matters because two bookmark managers will routinely create
the "same" bookmark under different strings. Without canonical
URLs, dedup is a heuristic; with canonical URLs, dedup is a
lookup.

## SQLite schema

The store is a single SQLite file. The schema (v2.2.0) has 7
tables:

| Table | Purpose |
|---|---|
| `bookmarks` | The bookmark records themselves |
| `tags` | Tag definitions (id + canonical name) |
| `bookmark_tags` | Many-to-many bookmark ↔ tag |
| `folders` | Folder hierarchy (for Chromium-style import) |
| `bookmark_folders` | Many-to-many bookmark ↔ folder |
| `sync_log` | Sync metadata (last seen, last pushed) |
| `schema_version` | Single-row schema version table |

### `bookmarks` columns

```text
id            ULID primary key
canonical_url TEXT NOT NULL  -- the canonical form (unique index)
original_url  TEXT NOT NULL  -- the first-seen raw URL
title         TEXT NOT NULL
notes         TEXT NULL
created_at    TEXT NOT NULL  -- RFC3339 timestamp
updated_at    TEXT NOT NULL
last_visit_at TEXT NULL      -- null if never visited
visit_count   INTEGER NOT NULL DEFAULT 0
```

The unique index on `canonical_url` enforces dedup at the storage
layer. An `INSERT` of a duplicate canonical URL fails with
`SQLITE_CONSTRAINT_UNIQUE`, which the importer treats as
"already-imported, skip".

## The dedup algorithm

`linkmarks dedupe` walks the entire `bookmarks` table, groups by
`canonical_url`, and for each group keeps one record per the
following tie-breakers (in order):

1. **Most recent `updated_at`** — the most-recently-touched record
   wins.
2. **Most recent `last_visit_at`** — among records with the same
   `updated_at`, the most-recently-visited wins.
3. **Highest `visit_count`** — among ties on timestamps, the
   most-visited wins.
4. **Lexicographically smallest `id`** — final deterministic
   fallback.

The losing records are deleted; their `bookmark_tags` and
`bookmark_folders` rows are re-parented to the winner before the
delete. The whole operation is a single transaction with
`PRAGMA foreign_keys = ON` (cascading deletes).

Re-running `linkmarks dedupe` against the same store produces
byte-identical output. This is verified by the property tests in
`linkmarks-core/src/dedupe.rs`.

## Sort modes

The TUI browser and the `linkmarks list` command both support four
sort modes:

| Mode | Comparator |
|---|---|
| `updated` | `updated_at DESC` (default) |
| `title` | `title COLLATE NOCASE ASC` |
| `canonical-url` | `canonical_url ASC` |
| `created` | `created_at DESC` |

The sort modes are enumerated as `SortMode` in `linkmarks-core`.
The TUI cycles through them with the `s` key; the CLI accepts
`--sort` as a flag.

## Filter modes

The TUI's filter input (`/` key) supports three filter modes:

| Mode | Description |
|---|---|
| `Substring` | Default. The query is matched as a substring against title + URL + tags. Case-insensitive. |
| `Tag` | The query is matched as a tag prefix (e.g. `rus` matches `rust`, `rust-cli`, `rustr` but not `cru`). |
| `Fuzzy` | The query is matched using nucleo's fuzzy matcher. `rs tmpl` matches `ratatui-template`. |

The filter modes are enumerated as `FilterMode` in
`linkmarks-tui`. The TUI cycles through them with `Ctrl+F`.

## Sync model (preview)

The self-hosted relay is preview-stage in v2.2.0. When enabled:

- Each device writes to its local SQLite store.
- `linkmarks sync push` serialises the changed rows into a
  per-collection yrs sub-document.
- The relay receives opaque yrs bytes + an HTTP path; it does not
  see plaintext bookmarks.
- Each devices pull merges the remote yrs bytes into its local
  store, applying the canonical-URL dedup on receive.

The relay itself is preview; the `linkmarks sync --remote` CLI is
fully wired and tested but the `linkmarks-relay` binary is in a
future release. The architecture is documented in
[Architecture](./architecture.md#sync).