# TUI browser

The TUI browser (`linkmarks tui`) is the primary interactive
surface. It runs in any terminal, uses ratatui for rendering and
crossterm for input, and indexes the store into an in-memory
nucleo matcher for sub-millisecond filter feedback.

## Layout

```text
┌──────────────────────────────────────────────────────────────────────┐
│ LinkMarks ~/local/share/linkmarks/linkmarks.db                       │
├──────────────────────────────────────────────────────────────────────┤
│ / rust tmpl ▏                                                          │
├──────────────────────────────────────────────────────────────────────┤
│ ▎ Updated 2026-08-12  rust-template  https://github.com/.../tmpl      │
│ ▎ Updated 2026-08-10  rust-analyzer  https://rust-analyzer.github.io │
│   Updated 2026-08-05  rust-2024     https://doc.rust-lang.org/...    │
│ ...                                                                    │
├──────────────────────────────────────────────────────────────────────┤
│ 1/1247 updated:rust,cli  s:sort  Ctrl+F:filter  ?:help  q:quit        │
└──────────────────────────────────────────────────────────────────────┘
```

The TUI has 5 regions: header (store path), filter bar, list,
status bar. The list selection cursor (`▎`) can be moved with
`j`/`k` or arrow keys.

## Keymap

| Key | Action |
|---|---|
| `j` / `↓` | Move cursor down |
| `k` / `↑` | Move cursor up |
| `g` | Jump to first record |
| `G` | Jump to last record |
| `/` | Enter filter mode |
| `Esc` | Clear filter, exit help/menu |
| `Enter` | Open the highlighted URL in the default browser |
| `Tab` | Toggle tag picker |
| `s` | Cycle sort mode |
| `Ctrl+F` | Cycle filter mode |
| `a` | Add a bookmark (prompts for URL + title) |
| `e` | Edit the highlighted bookmark |
| `d` | Delete the highlighted bookmark (with confirmation) |
| `r` | Refresh from disk |
| `?` | Toggle help overlay |
| `q` / `Ctrl+C` | Quit |

## Filter modes

Pressing `Ctrl+F` cycles through three modes; the active mode is
shown in the status bar.

### Substring (default)

The query is matched as a substring against title, URL, and tags.
Case-insensitive. Use this for direct keyword search.

Examples:

- `rust` matches every bookmark whose title, URL, or tags contain
  "rust" (case-insensitive)
- `cli template` matches records containing both substrings (in
  any field)

### Tag

The query is matched as a tag prefix. Useful for narrowing by
categorisation without typing the full tag.

Examples:

- `ru` matches tags `rust`, `rust-cli`, `rustr` but NOT `cru`
- `tech/` matches tags `tech/rust`, `tech/go` (forward-slash is
  literal)

### Fuzzy

The query is matched using nucleo's fuzzy matcher. Allows
out-of-order tokens and small typos.

Examples:

- `rs tmpl` matches `rust-template`, `ratatui-template`,
  `crate-template-rs`
- `linkmark` matches `linkmarks`, `link-marker`,
  `l-i-n-k-marks`

## Sort modes

Pressing `s` cycles through four modes; the active mode is shown
in the status bar.

| Mode | Comparator |
|---|---|
| `updated` | `updated_at DESC` (default) |
| `title` | `title COLLATE NOCASE ASC` |
| `canonical-url` | `canonical_url ASC` |
| `created` | `created_at DESC` |

The sort persists across filter changes within the same session.

## Actions

### Open URL

`Enter` opens the highlighted URL via `xdg-open` (Linux),
`open` (macOS), or `start` (Windows). If the URL is malformed,
the TUI shows a notification and continues.

### Add a bookmark

`a` opens an input prompt for URL; pressing `Enter` opens a
second prompt for title (with the URL as default). Tags are added
by `Tab`-completion against existing tags.

### Edit a bookmark

`e` opens an edit form for the highlighted record: title, notes,
tags. Save with `Enter`; cancel with `Esc`.

### Delete a bookmark

`d` opens a confirmation prompt. Default is `No` (cursor on `No`).
Press `Tab` to switch to `Yes`, then `Enter` to delete. The
delete is reversible: `u` (undo) restores the last 10 deletes
within the session.

## Themes

The TUI supports 4 themes, switchable with `:theme <name>` from
the help overlay:

- `rust` (default): the rust-lang.org palette
- `light`: high-contrast light theme
- `dark`: low-contrast dark theme
- `ayu`: Ayu-inspired theme

## Configuration

The TUI reads its keymap and theme from the global config. To
customise the keymap, write a `~/.config/linkmarks/keymap.toml`:

```toml
[keymap]
quit = ["q", "Ctrl+C", "Esc"]
open = ["Enter", "o"]
delete = ["d", "Backspace"]
```

The default keymap is documented in `linkmarks-tui/src/input.rs`.