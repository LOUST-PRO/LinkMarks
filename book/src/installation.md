# Getting started

This page covers installing the `linkmarks` binary, performing the
first-time store initialisation, and importing your first batch of
bookmarks.

## Install via cargo

```bash
cargo install linkmarks-cli \
    --git https://github.com/LOUST-PRO/LinkMarks \
    --tag v2.2.0 \
    --path crates/linkmarks-cli \
    --bin linkmarks \
    --locked
```

This places the binary at `~/.cargo/bin/linkmarks`. The `--locked`
flag pins the install to the lockfile shipped in the published
crate, ensuring the same dependency graph that CI tests. The
`--path` flag tells cargo which workspace member to build (the
repo is a Cargo workspace with 8 crates).

## Install from a package manager

### Arch Linux

```bash
pacman -U linkmarks-2.2.0-1-x86_64.pkg.tar.zst
```

The PKGBUILD lives at `arch/PKGBUILD` in the repo and ships a
`linkmarks.install` script with `pre_upgrade` and `post_install`
hooks for migrating the SQLite store on version bumps.

### Debian / Ubuntu

```bash
sudo dpkg -i linkmarks_2.2.0-1_amd64.deb
```

The Debian rules at `debian/rules` use `dh $@ --buildsystem cargo`
with a `dh-cargo` integration, and respect `DEB_BUILD_OPTIONS=nocheck`
in the test override.

### Fedora / RHEL

```bash
sudo dnf install ./linkmarks-2.2.0-1.fc42.x86_64.rpm
```

The spec at `rpm/linkmarks.spec` uses `rpmlint`-clean license
metadata (`License: AGPL-3.0-or-later`).

### Homebrew

```bash
brew install LOUST-PRO/tap/linkmarks
```

The Formula at `homebrew/Formula/linkmarks.rb` uses
`brew audit --strict`-clean metadata.

## First-time init

The first run creates the XDG store directory and a default config:

```bash
linkmarks init
# expected output: "Initialised linkmarks store at ~/.local/share/linkmarks/linkmarks.db"
#                  "Wrote default config to ~/.config/linkmarks/config.toml"
```

The default config (`config.toml.example` shows every field with
comments) covers:

- The SQLite path (default: XDG-resolved)
- The default sort mode (one of `updated`, `title`, `canonical-url`,
  `created`)
- The default filter mode (1 of 3 — see [Concepts](./concepts.md))
- The sync relay URL (only used when `linkmarks sync` runs)

`linkmarks init` is idempotent — re-running it is safe.

## Importing your first bookmarks

### From Chromium (Brave, Edge, Opera, etc.)

Chromium-family browsers store bookmarks at
`<profile>/Bookmarks` as a JSON file. Find your profile with
`chrome://version` (the "Profile Path" line).

```bash
linkmarks import chromium ~/.config/chromium/Default/Bookmarks
# expected output: "Imported 1247 bookmarks from chromium"
#                  "  - 1189 kept after canonical-URL dedupe"
#                  "  - 58 duplicates suppressed"
```

The importer:

1. Parses Chromium's `bookmark_bar`, `other`, and `synced` folders.
2. Resolves Chromium's internal node IDs to deterministic ULIDs.
4. Canonicalises every URL (sorted query params, lowercased host,
   no fragment).
5. Deduplicates by canonical URL — last write wins per URL.
6. Imports folder structure as tag-prefix tags (e.g.
   `bar/Tech/Rust` becomes `["bar", "Tech", "Rust"]`).

### From Firefox

Firefox stores bookmarks at `<profile>/places.sqlite`. Find your
profile with `about:profiles`.

```bash
linkmarks import firefox ~/.mozilla/firefox/abc123.default-release/places.sqlite
```

The Firefox bridge reads `moz_bookmarks` (folder structure) and
`moz_places` (URL/title/timestamp). It does NOT import `moz_annos`
(annotations) by default; pass `--with-annotations` to include
them.

### From Netscape HTML (Pocket, Raindrop, Pinboard exports)

```bash
linkmarks import netscape ~/Downloads/pocket-export.html
```

The Netscape bridge parses the standard `<DL>` / `<A HREF>`
hierarchy. Tags from `<DD>` comments are preserved.

## Verify the install

Two sanity checks confirm the store is alive:

```bash
linkmarks --version
# expected: linkmarks 2.2.0

linkmarks list --limit 5
# expected: table with the 5 most-recently-updated bookmarks
```

If `linkmarks list` returns `Error::StoreNotInitialised`, run
`linkmarks init` first.

## Uninstalling

```bash
cargo uninstall linkmarks-cli
rm -rf ~/.local/share/linkmarks
rm -rf ~/.config/linkmarks
```

The SQLite store is the only on-disk state. Removal is fully
reversible.