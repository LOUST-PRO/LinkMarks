//! `BookmarkSource` implementation backed by a Chromium `Bookmarks`
//! JSON file.
//!
//! Discovery: by default the source opens the path the caller
//! provides. A helper `discover_default_paths` returns the typical
//! locations for Chrome / Brave / Edge / Arc / Vivaldi / Opera on
//! Linux for v1.

use crate::parser::{parse_and_flatten, ParseError};
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, SourceKind};
use linkmarks_core::traits::{BookmarkSource, Page};
use std::path::{Path, PathBuf};

/// A read-only `BookmarkSource` for a Chromium `Bookmarks` file.
pub struct ChromiumSource {
    path: PathBuf,
    /// Cached parse result. `None` means not yet loaded.
    cache: Option<Vec<Bookmark>>,
}

impl ChromiumSource {
    /// Open a Chromium `Bookmarks` JSON file.
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        if !path.exists() {
            return Err(CoreError::InvalidUrl(format!(
                "file not found: {}",
                path.display()
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
            cache: None,
        })
    }

    /// Force a re-read on the next `list()` call.
    pub fn invalidate(&mut self) {
        self.cache = None;
    }
}

impl BookmarkSource for ChromiumSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Chromium
    }

    fn list(&self) -> Result<Vec<Bookmark>, CoreError> {
        // Re-parse each call: no cache to keep the source semantically
        // stateless. (`ChromiumSource::load` is private and reserved
        // for future caching once IO cost matters.)
        let (bookmarks, _errors) = parse_and_flatten(&self.path).map_err(|e| match e {
            ParseError::Io(io) => CoreError::Io(io),
            ParseError::Json(j) => CoreError::Json(j),
            other => CoreError::Canonicalize(other.to_string()),
        })?;
        Ok(bookmarks)
    }

    fn list_paginated(&self, _cursor: Option<String>, limit: usize) -> Result<Page, CoreError> {
        let all = self.list()?;
        let items: Vec<Bookmark> = all.into_iter().take(limit).collect();
        Ok(Page {
            items,
            next_cursor: None,
        })
    }

    fn by_canonical(&self, canonical: &str) -> Result<Option<Bookmark>, CoreError> {
        let all = self.list()?;
        Ok(all.into_iter().find(|b| b.canonical_url == canonical))
    }
}

/// Returns the typical default locations for Chromium-family browsers
/// on Linux. v1 ships Linux paths only.
///
/// Returned paths are not guaranteed to exist; the caller decides
/// whether to filter on `path.exists()`.
#[must_use]
pub fn discover_default_paths() -> Vec<(SourceKind, PathBuf)> {
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => return Vec::new(),
    };

    let profiles = ["Default", "Profile 1", "Profile 2", "Profile 3"];

    let browsers: &[(&str, &str)] = &[
        ("google-chrome", "Chrome"),
        ("BraveSoftware/Brave-Browser", "Brave"),
        ("microsoft-edge", "Edge"),
        ("arc", "Arc"),
        ("vivaldi", "Vivaldi"),
        ("opera", "Opera"),
    ];

    let mut out = Vec::new();
    for (config_subdir, _name) in browsers {
        let base = home.join(".config").join(config_subdir);
        for profile in profiles {
            let p = base.join(profile).join("Bookmarks");
            if p.exists() {
                out.push((SourceKind::Chromium, p));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_missing_path_errors() {
        let res = ChromiumSource::open(Path::new("/nonexistent/path/Bookmarks"));
        assert!(res.is_err());
    }
}
