# linkmarks-bridge-netscape

Bridge for the standard Netscape bookmark HTML format. The format
itself is the universal interchange layer used by Firefox, Chrome,
Edge, Safari, Pinboard, Linkwarden, Shaarli, and most self-hosted
bookmark managers. It is plain HTML with `<A HREF=...>` and `<H3>` /
`<DD>` markup.

This bridge handles both directions:

- **Import**: parse a Netscape HTML file into `Bookmark` records.
- **Export**: render `Bookmark` records into a Netscape HTML file.

It is the only bridge in the workspace that supports export. The
others are import-only.

## What is it

`linkmarks-netscape` parses the de-facto bookmark HTML format using
a streaming XML reader (`quick-xml`). It folds the deeply-nested
folder structure back into a flat list of bookmarks while preserving
the folder hierarchy in each `Bookmark` record's `tags` field.

The exporter is symmetric: `Vec<Bookmark>` in, well-formed HTML out,
with the special top-level `<META HTTP-EQUIV="Content-Type"
CONTENT="text/html; charset=UTF-8">` header that every browser
expects.

## Install

```toml
[dependencies]
linkmarks-bridge-netscape = "2.2.0"
```

## Usage

Import:

```rust
use linkmarks_netscape::import;

let bookmarks = import(Path::new("./bookmarks.html"))?;
for bookmark in bookmarks {
    println!("{} -> {}", bookmark.title, bookmark.canonical_url);
}
```

Export:

```rust
use linkmarks_netscape::export;

let bookmarks: Vec<Bookmark> = store.list_all()?;
export(&bookmarks, Path::new("./bookmarks.html"))?;
```

The fixture used by the workspace tests is in
`tests/fixtures/netscape-bookmarks.example.html`. It exercises nested
folders, special characters in titles, and the empty-folder edge
case.

## Compatibility

The Netscape bookmark format has no formal version. The bridge
follows the modern Firefox export shape (with `ADD_DATE`,
`LAST_MODIFIED`, and `TAGS` attributes) and tolerates the older
Chrome / Safari shape that omits `LAST_MODIFIED`.

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md` at the project root. Contact:
`opensource@loust.pro`.
