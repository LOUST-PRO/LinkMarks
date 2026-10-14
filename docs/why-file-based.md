# Why file-based import (not live write-back)?

LinkMarks exports bookmarks as a portable file that the user imports into
their browser via the native "Import bookmarks" UI. This document
explains why that pattern is intentional and why live write-back is not
used.

## TL;DR

Browsers do not watch their own bookmarks file. If LinkMarks rewrites
`~/.config/vivaldi/Default/Bookmarks` while Vivaldi is running, the
browser ignores the change and silently overwrites the file on the
next save. File-based import is the only pattern that actually
synchronises state, and it is the same pattern the browser uses
internally.

## How the file-based pattern works

1. `linkmarks export --format=chrome --output ~/bookmarks.json` writes
   a Chromium Bookmarks JSON file (atomic via `tempfile`).
2. The user opens their browser and runs the native import flow:
   - Vivaldi / Chrome / Edge / Brave / Arc / Opera:
     Bookmarks → Bookmark manager → ⋮ → Import bookmarks.
   - Firefox: Show All Bookmarks (Ctrl+Shift+O) → Import and Backup →
     Import Bookmarks from HTML.
3. The browser reads the JSON, merges it into its in-memory
   `BookmarkModel`, and persists atomically to its own file.

The import step is unavoidable. It is also idempotent — re-importing
the same file produces the same in-browser state, modulo any new
bookmarks the user has added in the meantime.

## Why live write-back is not viable

### Chromium does not watch the Bookmarks file

The Chromium `BookmarkModel` is loaded once at browser startup and
held entirely in memory. There is no inotify watcher, no polling
loop, no IPC endpoint for an external process to notify. Every
change to the bookmarks set goes through the in-memory model; on
save, the model serialises itself back to the file, overwriting
whatever was there.

If LinkMarks rewrites the file while the browser is running:

1. The atomic `rename` swaps the inode under the browser's nose.
2. The browser's `BookmarkModel` still holds the old data in memory.
3. The user makes any change in the browser (add, edit, delete) and
   saves. The browser serialises its in-memory state to the file,
   overwriting LinkMarks' write.
4. Result: silent data loss. LinkMarks' effort is gone without an
   error message.

This is not a LinkMarks limitation — it is how Chromium works.

### There is no public IPC endpoint

Chrome DevTools Protocol exposed `Page.addBookmark` historically, but
it was deprecated in Chrome 110+ (removed from the public schema).
The remaining options are:

| Option | Status |
|---|---|
| `Page.addBookmark` (CDP) | Deprecated, removed from Chrome 110+ |
| `chrome.bookmarks` API (extension) | Requires a Web Store–published extension |
| Native messaging host | Requires install per machine, signed manifest |

A custom Chrome extension is the only viable path, but it is a
separate project (`linkmarks-bridge-extension` if it ever exists).
It requires Web Store review, install on every machine, and
cross-browser variants (Chrome, Edge, Brave, Arc, Vivaldi, Opera
each maintain their own extension policies).

### Auto-restart is destructive

The "kill the browser, write the file, reopen" pattern works, but it
kills all of the user's open tabs, in-memory session state, and
unsaved form data. For a bookmark manager — a low-stakes tool —
that is disproportionate. There is no way to restart Chromium and
restore the user's exact session; the browser would reopen to the
default new-tab page.

### CDP via headless shell is fragile

It is technically possible to launch a second instance of the
browser in headless mode, connect via CDP, and inject bookmarks
through `Runtime.evaluate`. This is fragile:

- The headless instance has a separate profile; bookmarks do not
  flow into the user's main browser.
- The CDP session must be kept alive, with reconnect logic on
  browser restarts.
- `Page.addBookmark` is gone; the only path is JS injection through
  `chrome.bookmarks` API, which still requires the extension
  approval flow.

## Alternatives LinkMarks considered

| Alternative | Verdict |
|---|---|
| File-based import (current) | ✅ Safe, idempotent, browser-aware |
| Live write-back via file rename | ❌ Browser ignores writes; data loss |
| CDP via headless shell | ❌ Fragile, separate profile, deprecated APIs |
| Custom Chrome extension | ⏸ Out of scope — separate project |
| Auto-restart browser | ❌ Destructive, kills user session |
| Replace browser bookmarks via `BookmarkManager` IPC | ❌ No public IPC; only via extension |

## How to use the file-based pattern

```bash
# 1. Export
linkmarks export --format=chrome --output ~/bookmarks.json

# 2. Open Vivaldi (or Chrome / Edge / Brave / Arc / Opera):
#    Bookmarks → Bookmark manager → ⋮ → Import bookmarks → ~/bookmarks.json

# 3. Done. Vivaldi reads the JSON, merges it, persists atomically.
```

```bash
# For Firefox (HTML format):
linkmarks export --format=netscape --output ~/bookmarks.html
# Then in Firefox: Show All Bookmarks → Import and Backup → Import
# Bookmarks from HTML → ~/bookmarks.html
```

## When to revisit

A future major version may add:

- A `linkmarks-bridge-extension` companion project that wraps the
  `chrome.bookmarks` API for true live sync (requires Web Store
  distribution).
- A Native Messaging host for Firefox (Mozilla has fewer
  restrictions on distribution than the Chrome Web Store).

Neither is on the LinkMarks roadmap today. File-based import is the
recommended pattern for the foreseeable future.

## Sources

- [chromium-discuss: Bookmark File External Editing](https://groups.google.com/a/chromium.org/g/chromium-discuss/c/3PZj1eVadNY) —
  community confirmation that external edits are silently overwritten
- [chromium.googlesource.com — bookmark_model_observer_impl.h](https://chromium.googlesource.com/chromium/src/+/b2075eb/components/sync_bookmarks/bookmark_model_observer_impl.h) —
  observer pattern is in-process only, not a filesystem watcher
- [Stack Overflow: Manipulating Chrome bookmarks from external program](https://stackoverflow.com/questions/5217994/manipulating-google-chrome-bookmarks-from-external-program-and-make-chrome-awar) —
  13 years of consensus that browsers do not watch the file
