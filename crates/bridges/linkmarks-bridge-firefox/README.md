# linkmarks-bridge-firefox

Bridge for Firefox bookmarks. Two input shapes are supported:

- Live profile: `places.sqlite` (read-only, attach + select).
- Browser-closed snapshot: `bookmarkbackups/*.jsonlz4` (LZ4-compressed
  JSON, decoded with `lz4_flex`).

This is the only LinkMarks bridge that reads a SQLite database directly
from the browser; the others read JSON or HTML files. The `places.sqlite`
we read is opened in SQLite read-only mode via the `mode=ro` URI on
every connection. We never write to the Firefox database.

## What is it

`linkmarks-firefox` implements the `BookmarkSource` trait twice — once
for the live profile and once for the JSONLZ4 backup files. The
public API exposes a single `FirefoxBridge::new()` that selects the
right reader based on the path it is given.

Both readers produce the same `Bookmark` records, so downstream code
does not care whether the data came from a live profile or a backup.

## Install

```toml
[dependencies]
linkmarks-bridge-firefox = "2.2.0"
```

## Usage

```rust
use linkmarks_bridge_firefox::FirefoxBridge;

let bridge = FirefoxBridge::new();

// Live profile (places.sqlite)
let bookmarks = bridge.read(Path::new(
    "/home/me/.mozilla/firefox/abc.default-release/places.sqlite",
))?;
```

To read a JSONLZ4 backup instead, pass the path to the `.jsonlz4`
file directly.

## Compatibility

Tested against Firefox 122+ place schema. The `moz_bookmarks` /
`moz_places` join pattern is stable across recent Firefox versions,
but the older `bookmarkbackups/*.json` (uncompressed) format is **not**
supported by this bridge — use the `linkmarks-bridge-netscape` exporter
for the universal interchange path.

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md` at the project root. Contact:
`opensource@loust.pro`.
