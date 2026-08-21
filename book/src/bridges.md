# Bridge formats

LinkMarks imports bookmarks from 3 source formats and exports to
2. Each bridge is a separate crate (`linkmarks-bridge-*`) with
its own dependency tree, its own test fixture, and its own
round-trip fidelity test.

## Chromium / Brave / Edge / Opera

The Chromium bridge (`linkmarks-bridge-chromium`) parses the
`Bookmarks` JSON file produced by Chromium-family browsers.

### File location

```text
Linux:    ~/.config/<browser>/Default/Bookmarks
macOS:    ~/Library/Application Support/<browser>/Default/Bookmarks
Windows:  %LOCALAPPDATA%\<browser>\User Data\Default\Bookmarks
```

The file is written by the browser on every bookmark change; the
importer reads it once and disconnects.

### Schema mapping

| Chromium field | LinkMarks field |
|---|---|
| `bookmark_bar.children[]` | records + folder `bar/` |
| `other.children[]` | records + folder `other/` |
| `synced.children[]` | records + folder `synced/` |
| `children[].url` | `original_url` (canonicalised on import) |
| `children[].name` | `title` |
| `children[].date_added` | `created_at` |
| `children[].date_last_used` | `last_visit_at` (when present) |
| `children[].id` | discarded (LinkMarks assigns its own ULID) |
| `children[].guid` | discarded |
| `children[].meta_info` | `notes` (when present) |
| Folder paths | `bookmark_folders` rows |

### What's preserved

- URL (canonicalised)
- Title
- Date added → `created_at`
- Date last used → `last_visit_at` (when non-zero)
- Folder hierarchy (the importer splits `/`-separated folder
  paths into ancestor folders)
- Tags from `meta_info` (when present)

### What's NOT preserved

- Favicon (not stored in the Bookmarks JSON)
- Visit count (Chromium does not embed this; the importer starts
  from 0 and the user's browsing history is not accessible)
- Sync metadata (the `synced` folder is preserved as a folder
  but the per-device attribution is not)
- Internal IDs (LinkMarks assigns its own ULIDs)

### Round-trip test

The Chromium bridge ships with a fixture
(`fixtures/chromium-v123.bookmarks`) and a round-trip test:

1. Import fixture → store A
2. Export store A as Netscape HTML
3. Import the Netscape HTML → store B
4. Compare A and B by canonical URL

The test passes when every record in A has a matching canonical
URL in B with the same title, tags, and folder path.

## Firefox

The Firefox bridge (`linkmarks-bridge-firefox`) reads a
`places.sqlite` SQLite file directly.

### File location

```text
Linux:    ~/.mozilla/firefox/<profile>/places.sqlite
macOS:    ~/Library/Application Support/Firefox/Profiles/<profile>/places.sqlite
Windows:  %APPDATA%\Mozilla\Firefox\Profiles\<profile>\places.sqlite
```

The bridge reads the file read-only; the importer does not
require Firefox to be closed.

### Schema mapping

| Firefox table.column | LinkMarks field |
|---|---|
| `moz_places.url` | `original_url` |
| `moz_places.title` | `title` |
| `moz_places.last_visit_date` | `last_visit_at` |
| `moz_bookmarks.dateAdded` | `created_at` |
| `moz_bookmarks.lastModified` | `updated_at` |
| `moz_bookmarks.title` | folder title (when `type = 2`) |
| `moz_bookmarks.type = 1` | bookmark record |
| `moz_bookmarks.type = 2` | folder record |
| `moz_bookmarks.type = 3` | separator (discarded) |
| `moz_annos.content` (when `--with-annotations`) | `notes` |

### What's preserved

- URL (canonicalised)
- Title
- Folder hierarchy
- Tags from `moz_tags` (when present; Firefox's tagging was
  introduced in Firefox 96)
- Annotations (when `--with-annotations`)
- Date added / date modified / date last visited

### What's NOT preserved

- Favicons (stored in `favicons.sqlite`, not in `places.sqlite`)
- Tags from `places.sqlite` without the
  `--with-annotations` flag (the import is much faster without
  annotations; the SQLite ATTACH is the expensive part)
- History (only bookmarks are imported; the importer filters by
  `moz_bookmarks.type IN (1, 2)`)
- Visit counts (not stored in `places.sqlite`; Firefox tracks
  visits separately in `moz_historyvisits`)

### Live Firefox warning

If Firefox is running while `linkmarks import firefox` executes,
the bridge reads the file in read-only mode. SQLite WAL mode is
honored; the importer does not block Firefox's writes.

## Netscape HTML

The Netscape bridge (`linkmarks-bridge-netscape`) parses the
canonical `<DL><DT><A HREF>` HTML format used by every browser
export ever and by Pocket / Raindrop / Pinboard exports.

### Example input

```html
<!DOCTYPE NETSCAPE-Bookmark-file-1>
<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">
<TITLE>Bookmarks</TITLE>
<H1>Bookmarks</H1>
<DL><p>
    <DT><H3 ADD_DATE="1723500000">Tech</H3>
    <DL><p>
        <DT><A HREF="https://example.com/" ADD_DATE="1723500001">Example</A>
        <DD>example-tag
        <DT><A HREF="https://github.com/" ADD_DATE="1723500002">GitHub</A>
    </DL><p>
</DL><p>
```

### Schema mapping

| Netscape attribute | LinkMarks field |
|---|---|
| `HREF` | `original_url` |
| Text content of `<A>` | `title` |
| `ADD_DATE` (epoch seconds) | `created_at` |
| `LAST_MODIFIED` (epoch seconds) | `updated_at` (when present) |
| `TAGS` (comma-separated, post-2008) | `tags` |
| Text content of `<DD>` (Pocket style) | `tags` (split by `,`) |
| `<H3>` headings | folder titles |
| Nested `<DL>` | folder hierarchy |

### What's preserved

- URL (canonicalised)
- Title
- Tags (from `TAGS` attribute or `<DD>` comment)
- Folder hierarchy (from `<H3>` headings)
- Date added / date modified

### What's NOT preserved

- Favicons (not in the Netscape format)
- Visit history (not in the Netscape format)
- Per-record GUIDs (LinkMarks assigns its own ULIDs)

### Round-trip test

The Netscape bridge's round-trip test is identical to Chromium's:
import → export → re-import → compare by canonical URL.

## Export formats

`linkmarks export <FORMAT>` writes the store in the chosen
format. Two formats are supported:

- `netscape` — produces a `<DL>` HTML file, ready for import
  into another browser or service
- `json` — produces a per-record JSON array, suitable for
  piping into `jq`
- `csv` — produces a CSV with columns matching the
  `linkmarks list --format csv` schema

## Choosing a bridge

If you have multiple source formats, import each one in order
of size:

1. Firefox (most complete: tags + annotations)
2. Chromium (most universally available)
3. Netscape HTML (least information, but the most portable)

The canonical-URL dedupe is idempotent, so importing multiple
sources is safe.