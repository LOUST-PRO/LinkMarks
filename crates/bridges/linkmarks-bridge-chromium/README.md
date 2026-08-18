# linkmarks-bridge-chromium

Bridge for Chromium-family Bookmarks JSON. Reads the
`Bookmarks` file that Chrome, Brave, Edge, Arc, Vivaldi, and Opera
write into the user profile. Read-only: never writes back to the
browser file.

The shape of the input is identical across all Chromium derivatives;
the only differences are the parent directory (`google-chrome/Default/`,
`Brave-Browser/Default/`, etc.) and the on-disk profile naming. This
crate handles the JSON shape; the umbrella tells it which path to
read.

## What is it

This bridge implements the `BookmarkSource` trait from `linkmarks-core`
on top of the Chromium JSON format. It does not walk the filesystem
looking for any profile — callers pass a path explicitly. That is
deliberate: it keeps the bridge hermetic and testable.

Output is a stream of `Bookmark` records with full provenance
(`source = "chrome"`, `imported_at = <UTC>`).

## Install

```toml
[dependencies]
linkmarks-bridge-chromium = "2.2.0"
```

## Usage

```rust
use linkmarks_bridge_chromium::ChromiumBridge;

let bridge = ChromiumBridge::new();
let bookmarks = bridge.read(Path::new(
    "/home/me/.config/google-chrome/Default/Bookmarks",
))?;
for bookmark in bookmarks {
    println!("{} -> {}", bookmark.title, bookmark.canonical_url);
}
```

The fixture used by the workspace tests is in
`tests/fixtures/chrome-bookmarks.example.json`. It exercises
nested folders, the `bookmark_bar` / `other` / `synced` top-level
containers, and the metainfo header.

## Compatibility

Tested against the Chrome 124+ JSON schema. Older schemas (Chrome 23
through 50) used a different top-level shape; those are not supported.

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md` at the project root. Contact:
`opensource@loust.pro`.
