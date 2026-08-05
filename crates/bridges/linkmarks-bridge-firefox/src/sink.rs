//! Safe Firefox-format JSON sink.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use linkmarks_core::errors::CoreError;
use linkmarks_core::model::{Bookmark, SourceKind};
use linkmarks_core::traits::{BookmarkSink, WriteReport};
use serde_json::to_vec_pretty;

use crate::tree::FirefoxNode;
use crate::FirefoxTree;

/// A sink that writes uncompressed Firefox-shaped JSON, never Places SQLite.
pub struct FirefoxJsonSink {
    path: PathBuf,
}

impl FirefoxJsonSink {
    /// Bind a sink to a JSON output path.
    #[must_use]
    pub fn open(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Build Firefox backup JSON without touching disk.
    pub fn build_tree(bookmarks: &[Bookmark]) -> FirefoxTree {
        let mut roots = BTreeSet::new();
        for bookmark in bookmarks {
            if let Some(collection) = &bookmark.collection {
                if let Some(root) = collection.split('/').next().filter(|part| !part.is_empty()) {
                    roots.insert(root.to_string());
                }
            }
        }
        let mut children = Vec::new();
        for root in roots {
            let parts = vec![root.as_str()];
            children.push(folder_node(&parts, bookmarks, 2));
        }
        for bookmark in bookmarks.iter().filter(|b| b.collection.is_none()) {
            children.push(bookmark_node(bookmark, 2));
        }
        FirefoxTree {
            guid: "root________".to_string(),
            title: String::new(),
            id: 1,
            type_code: 1,
            date_added: Some(now_micros()),
            last_modified: Some(now_micros()),
            children,
        }
    }

    /// Write atomically and return the serialized JSON.
    pub fn write_to(
        path: &Path,
        bookmarks: &[Bookmark],
    ) -> Result<(WriteReport, Vec<u8>), CoreError> {
        let body = to_vec_pretty(&Self::build_tree(bookmarks))?;
        write_atomic(path, &body)?;
        Ok((
            WriteReport {
                written: bookmarks.len(),
                failed: Vec::new(),
            },
            body,
        ))
    }
}

fn folder_node(parts: &[&str], bookmarks: &[Bookmark], id: i64) -> FirefoxNode {
    let prefix = parts.join("/");
    let direct = bookmarks
        .iter()
        .filter(|b| b.collection.as_deref() == Some(prefix.as_str()))
        .map(|b| bookmark_node(b, id + 1));
    let mut children: Vec<FirefoxNode> = direct.collect();
    let mut nested = BTreeMap::<String, Vec<&Bookmark>>::new();
    for bookmark in bookmarks {
        if let Some(collection) = &bookmark.collection {
            if collection.starts_with(&(prefix.clone() + "/")) {
                if let Some(next) = collection
                    .strip_prefix(&(prefix.clone() + "/"))
                    .and_then(|s| s.split('/').next())
                {
                    nested.entry(next.to_string()).or_default().push(bookmark);
                }
            }
        }
    }
    for name in nested.keys() {
        let mut next = parts.to_vec();
        next.push(name);
        children.push(folder_node(&next, bookmarks, id + 1));
    }
    FirefoxNode {
        guid: format!("folder-{id}-{}", crate::tree::folder_slug(&prefix)),
        title: parts.last().unwrap_or(&"").to_string(),
        id,
        type_code: 1,
        uri: None,
        date_added: Some(now_micros()),
        last_modified: Some(now_micros()),
        tags: Vec::new(),
        children,
    }
}

fn bookmark_node(bookmark: &Bookmark, id: i64) -> FirefoxNode {
    FirefoxNode {
        guid: bookmark
            .source
            .external_id
            .clone()
            .unwrap_or_else(|| format!("link-{}", bookmark.id.0)),
        title: bookmark.title.clone(),
        id,
        type_code: 2,
        uri: Some(bookmark.original_url.clone()),
        date_added: Some(bookmark.created_at.timestamp_micros()),
        last_modified: Some(bookmark.updated_at.timestamp_micros()),
        tags: bookmark
            .tags
            .iter()
            .filter(|tag| !tag.starts_with("#folder/"))
            .cloned()
            .collect(),
        children: Vec::new(),
    }
}

fn now_micros() -> i64 {
    Utc::now().timestamp_micros()
}
fn write_atomic(path: &Path, body: &[u8]) -> Result<(), CoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| CoreError::Storage(format!("firefox sink temp-file: {e}")))?;
    tmp.write_all(body)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| CoreError::Io(e.error))?;
    Ok(())
}

impl BookmarkSink for FirefoxJsonSink {
    fn kind(&self) -> SourceKind {
        SourceKind::Firefox
    }
    fn write(&mut self, bookmarks: &[Bookmark]) -> Result<WriteReport, CoreError> {
        Self::write_to(&self.path, bookmarks).map(|(report, _)| report)
    }
    fn delete(&mut self, _external_id: &str) -> Result<(), CoreError> {
        Err(CoreError::Storage(
            "Firefox places.sqlite is read-only; JSON sink delete requires a rewrite batch"
                .to_string(),
        ))
    }
}
