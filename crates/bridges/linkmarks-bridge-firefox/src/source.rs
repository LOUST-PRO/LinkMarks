//! `BookmarkSource` over Firefox Places or backup snapshots.

use crate::errors::BridgeError;
use crate::{jsonlz4, places};
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, SourceKind};
use linkmarks_core::traits::{BookmarkSource, Page};
use std::path::{Path, PathBuf};

/// A read-only Firefox source.
#[derive(Debug, Clone)]
pub enum FirefoxSource {
    /// A live profile database; it is always opened read-only.
    Places {
        /// Path to the live Places database.
        path: PathBuf,
    },
    /// A browser-closed compressed backup.
    Jsonlz4 {
        /// Path to the jsonlz4 backup.
        path: PathBuf,
    },
}

impl FirefoxSource {
    /// Create a source backed by a Places database.
    pub fn from_places_path(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(CoreError::InvalidUrl(format!(
                "file not found: {}",
                path.display()
            )));
        }
        Ok(Self::Places {
            path: path.to_path_buf(),
        })
    }

    /// Create a source backed by a jsonlz4 snapshot.
    pub fn from_jsonlz4_path(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(CoreError::InvalidUrl(format!(
                "file not found: {}",
                path.display()
            )));
        }
        Ok(Self::Jsonlz4 {
            path: path.to_path_buf(),
        })
    }

    /// Return existing Firefox Places and backup files in conventional profiles.
    #[must_use]
    pub fn discover_default_paths() -> Vec<(SourceKind, PathBuf)> {
        discover_default_paths()
    }

    fn load(&self) -> Result<Vec<Bookmark>, BridgeError> {
        match self {
            Self::Places { path } => places::parse_places(path),
            Self::Jsonlz4 { path } => Ok(jsonlz4::parse_file(path)?.flatten()),
        }
    }
}

impl BookmarkSource for FirefoxSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Firefox
    }

    fn list(&self) -> Result<Vec<Bookmark>, CoreError> {
        let mut list = self.load().map_err(CoreError::from)?;
        list.sort_by(|a, b| {
            a.canonical_url
                .cmp(&b.canonical_url)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        Ok(list)
    }

    fn list_paginated(&self, cursor: Option<String>, limit: usize) -> Result<Page, CoreError> {
        let list = self.list()?;
        let start = cursor
            .as_deref()
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| CoreError::Storage("invalid Firefox cursor".to_string()))?;
        let end = start.saturating_add(limit).min(list.len());
        Ok(Page {
            items: list[start.min(list.len())..end].to_vec(),
            next_cursor: (end < list.len()).then(|| end.to_string()),
        })
    }

    fn by_canonical(&self, canonical: &str) -> Result<Option<Bookmark>, CoreError> {
        Ok(self
            .list()?
            .into_iter()
            .find(|bookmark| bookmark.canonical_url == canonical))
    }
}

/// Discover existing Firefox profile stores on Linux and macOS.
#[must_use]
pub fn discover_default_paths() -> Vec<(SourceKind, PathBuf)> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let bases = [
        home.join(".mozilla/firefox"),
        home.join("Library/Application Support/Firefox/Profiles"),
    ];
    let mut found = Vec::new();
    for base in bases {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let profile = entry.path();
            let places = profile.join("places.sqlite");
            if places.is_file() {
                found.push((SourceKind::Firefox, places));
            }
            let backup_dir = profile.join("bookmarks-backups");
            if let Ok(backups) = std::fs::read_dir(backup_dir) {
                for backup in backups.flatten() {
                    let path = backup.path();
                    if path.extension().is_some_and(|ext| ext == "jsonlz4") {
                        found.push((SourceKind::Firefox, path));
                    }
                }
            }
        }
    }
    found
}
