//! Source dispatch helper for `linkmarks list`/`import`/`export`/`dedupe`.
//!
//! The four subcommands accept `--source=<kind>` and a `--path=<file>`.
//! The historical v1 wiring only knew about Chromium; v2 adds the
//! Firefox and Netscape bridges so the CLI can ingest bookmark stores
//! from any major browser family. Each bridge exposes a different
//! `open()` constructor (e.g. Firefox distinguishes `places.sqlite`
//! from compressed `jsonlz4` snapshots), so the dispatch lives in
//! one place rather than being copy-pasted across the four cmd files.
//!
//! `Pinboard` / `Linkwarden` are not importable from this CLI build
//! because those bridges ship in a separate workspace member with
//! network-stack dependencies. Surfacing a clear error here is
//! preferable to silently dropping the request.

use anyhow::{bail, Result};
use linkmarks_core::model::{Bookmark, SourceKind};
use linkmarks_core::traits::BookmarkSource;
use std::path::Path;

/// Open the source for the given kind at the given path and return
/// the parsed bookmark set.
///
/// The caller is responsible for verifying that the kind is one of the
/// importable variants (`Chromium`, `Firefox`, `Netscape`). Passing a
/// non-importable kind here is a programming error and returns a
/// clear `bail!` rather than a panic.
pub fn open_source(kind: SourceKind, path: &Path) -> Result<Vec<Bookmark>> {
    match kind {
        SourceKind::Chromium => {
            let src = linkmarks_bridge_chromium::ChromiumSource::open(path)?;
            Ok(src.list()?)
        }
        SourceKind::Firefox => {
            let src = open_firefox(path)?;
            Ok(src.list()?)
        }
        SourceKind::Netscape => {
            let src = linkmarks_bridge_netscape::NetscapeSource::open(path)?;
            Ok(src.list()?)
        }
        SourceKind::Pinboard | SourceKind::Linkwarden | SourceKind::Manual => {
            bail!(
                "--source={:?} is not importable from this CLI build; use `store` or one of `chrome`/`firefox`/`netscape`",
                kind
            )
        }
    }
}

/// Open a Firefox source. The bridge distinguishes live `places.sqlite`
/// profiles from browser-closed `*.jsonlz4` snapshots; we route by
/// extension so the caller does not need to know the difference.
fn open_firefox(path: &Path) -> Result<linkmarks_bridge_firefox::FirefoxSource> {
    let ext = path.extension().and_then(|e| e.to_str());
    match ext {
        Some("jsonlz4") => Ok(linkmarks_bridge_firefox::FirefoxSource::from_jsonlz4_path(path)?),
        // Default to places.sqlite — the most common Firefox profile store.
        _ => Ok(linkmarks_bridge_firefox::FirefoxSource::from_places_path(path)?),
    }
}

/// True if the kind can be opened from a path on disk by this CLI.
#[must_use]
pub fn is_path_source(kind: SourceKind) -> bool {
    matches!(
        kind,
        SourceKind::Chromium | SourceKind::Firefox | SourceKind::Netscape
    )
}

/// The set of `--source=<kind>` aliases the CLI accepts (lowercase).
///
/// `SourceKind::from_cli_str` already accepts a wider set (browser
/// aliases like `brave`/`vivaldi` collapse to `Chromium`); this list
/// only enumerates the canonical forms for help text.
pub const PATH_SOURCE_KINDS: &[&str] = &["chrome", "firefox", "netscape"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_path_source_true_for_three_kinds() {
        assert!(is_path_source(SourceKind::Chromium));
        assert!(is_path_source(SourceKind::Firefox));
        assert!(is_path_source(SourceKind::Netscape));
    }

    #[test]
    fn is_path_source_false_for_remote_or_manual_kinds() {
        assert!(!is_path_source(SourceKind::Pinboard));
        assert!(!is_path_source(SourceKind::Linkwarden));
        assert!(!is_path_source(SourceKind::Manual));
    }

    #[test]
    fn open_source_errors_on_missing_file() {
        let result = open_source(
            SourceKind::Chromium,
            &PathBuf::from("/nonexistent/path/Bookmarks"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn open_source_bails_on_unsupported_kind() {
        // `Manual` is not a path-backed source.
        let result = open_source(SourceKind::Manual, &PathBuf::from("/tmp/whatever"));
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("not importable"));
    }

    #[test]
    fn path_source_kinds_listed_for_help() {
        assert!(PATH_SOURCE_KINDS.contains(&"chrome"));
        assert!(PATH_SOURCE_KINDS.contains(&"firefox"));
        assert!(PATH_SOURCE_KINDS.contains(&"netscape"));
        assert_eq!(PATH_SOURCE_KINDS.len(), 3);
    }
}