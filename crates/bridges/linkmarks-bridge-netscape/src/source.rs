//! `BookmarkSource` implementation backed by a Netscape bookmark
//! HTML file.
//!
//! The source is read-only; writes go through `NetscapeSink`.
//! Discovery: by default the source opens the path the caller
//! provides. The helper `discover_default_paths` returns the
//! typical locations on Linux.

use std::path::{Path, PathBuf};

use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, SourceKind};
use linkmarks_core::traits::{BookmarkSource, Page};

use crate::parser::{parse_and_flatten, ParseError};

/// A read-only `BookmarkSource` for a Netscape bookmark HTML file.
pub struct NetscapeSource {
    path: PathBuf,
}

impl NetscapeSource {
    /// Open a Netscape bookmark HTML file. The path must exist;
    /// callers opening a default-discovered path can run
    /// `path.exists()` first.
    pub fn open(path: &Path) -> Result<Self, CoreError> {
        if !path.exists() {
            return Err(CoreError::InvalidUrl(format!(
                "file not found: {}",
                path.display()
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }

    /// Borrow the configured path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl BookmarkSource for NetscapeSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Netscape
    }

    fn list(&self) -> Result<Vec<Bookmark>, CoreError> {
        let parsed = parse_and_flatten(&self.path).map_err(parse_err_to_core)?;
        Ok(parsed.bookmarks)
    }

    fn list_paginated(&self, _cursor: Option<String>, limit: usize) -> Result<Page, CoreError> {
        let mut all = self.list()?;
        all.sort_by(|a, b| a.canonical_url.cmp(&b.canonical_url));
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

/// Map a `ParseError` into `CoreError`.
#[must_use]
fn parse_err_to_core(err: ParseError) -> CoreError {
    match err {
        ParseError::Io(io) => CoreError::Io(io),
        ParseError::Xml(s) => CoreError::Storage(format!("netscape xml: {s}")),
        ParseError::Attr(a) => CoreError::Storage(format!("netscape attr: {a}")),
        ParseError::Partial { element, reason } => CoreError::Partial { element, reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_missing_path_errors() {
        let res = NetscapeSource::open(Path::new("/nonexistent/path/bookmarks.html"));
        assert!(res.is_err());
    }
}
