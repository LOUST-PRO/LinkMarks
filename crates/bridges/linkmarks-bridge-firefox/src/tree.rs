//! Recursive representation of Firefox bookmark backup JSON.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use serde::{Deserialize, Serialize};

/// Top-level Firefox bookmark backup tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirefoxTree {
    /// Stable Firefox GUID.
    pub guid: String,
    /// Root title (normally empty).
    #[serde(default)]
    pub title: String,
    /// Firefox numeric row identifier.
    #[serde(default)]
    pub id: i64,
    /// Node type (`1` for folder).
    #[serde(rename = "typeCode", default = "folder_type")]
    pub type_code: u8,
    /// Creation time in Unix microseconds.
    #[serde(rename = "dateAdded", default, skip_serializing_if = "Option::is_none")]
    pub date_added: Option<i64>,
    /// Modification time in Unix microseconds.
    #[serde(
        rename = "lastModified",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_modified: Option<i64>,
    /// Root children.
    #[serde(default)]
    pub children: Vec<FirefoxNode>,
}

/// One folder or bookmark in a Firefox backup tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirefoxNode {
    /// Stable Firefox GUID.
    pub guid: String,
    /// Display title.
    #[serde(default)]
    pub title: String,
    /// Firefox numeric row identifier.
    #[serde(default)]
    pub id: i64,
    /// Node type (`1` folder, `2` bookmark).
    #[serde(rename = "typeCode")]
    pub type_code: u8,
    /// Bookmark URI, absent for folders.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// Creation time in Unix microseconds.
    #[serde(rename = "dateAdded", default, skip_serializing_if = "Option::is_none")]
    pub date_added: Option<i64>,
    /// Modification time in Unix microseconds.
    #[serde(
        rename = "lastModified",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_modified: Option<i64>,
    /// Modern Firefox bookmark tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Folder children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<FirefoxNode>,
}

const fn folder_type() -> u8 {
    1
}

impl FirefoxTree {
    /// Flatten the recursive Firefox tree into LinkMarks bookmarks.
    #[must_use]
    pub fn flatten(&self) -> Vec<Bookmark> {
        let mut out = Vec::new();
        for child in &self.children {
            child.flatten_into(&[], &mut out, true);
        }
        out
    }
}

impl FirefoxNode {
    fn flatten_into(&self, ancestors: &[String], out: &mut Vec<Bookmark>, root_child: bool) {
        if self.type_code == 1 {
            let title = if root_child {
                standard_root_name(self.id, &self.title)
            } else {
                self.title.trim().to_string()
            };
            if title.to_ascii_lowercase().starts_with("tag:") {
                for child in &self.children {
                    child.flatten_into(ancestors, out, false);
                }
                return;
            }
            let mut next = ancestors.to_vec();
            if !title.is_empty() {
                next.push(title);
            }
            for child in &self.children {
                child.flatten_into(&next, out, false);
            }
            return;
        }
        if self.type_code != 2 {
            return;
        }
        let Some(url) = self.uri.as_deref().filter(|url| !url.is_empty()) else {
            return;
        };
        let Ok(canonical_url) = linkmarks_core::canonicalize(url) else {
            return;
        };
        let created_at = timestamp(self.date_added.or(self.last_modified));
        let updated_at = timestamp(self.last_modified.or(self.date_added));
        let mut tags: BTreeSet<String> = self
            .tags
            .iter()
            .map(|tag| tag.trim().to_ascii_lowercase())
            .filter(|tag| !tag.is_empty())
            .collect();
        for folder in ancestors {
            tags.insert(format!("#folder/{}", folder_slug(folder)));
        }
        out.push(Bookmark {
            id: BookmarkId::generate(),
            original_url: url.to_string(),
            canonical_url,
            title: self.title.trim().to_string(),
            description: None,
            tags: tags.into_iter().collect(),
            collection: (!ancestors.is_empty()).then(|| ancestors.join("/")),
            created_at,
            updated_at,
            source: SourceRef {
                kind: SourceKind::Firefox,
                external_id: Some(self.guid.clone()),
                imported_at: Utc::now(),
                raw: None,
            },
            content_type: None,
            archived: false,
        });
    }
}

fn timestamp(value: Option<i64>) -> DateTime<Utc> {
    value
        .and_then(DateTime::from_timestamp_micros)
        .unwrap_or_default()
}

fn standard_root_name(id: i64, title: &str) -> String {
    match id {
        2 => "Bookmarks Menu".to_string(),
        3 => "Bookmarks Toolbar".to_string(),
        5 => "Other Bookmarks".to_string(),
        _ => title.trim().to_string(),
    }
}

pub(crate) fn folder_slug(folder: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for ch in folder.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            separator = false;
        } else if !separator && !slug.is_empty() {
            slug.push('-');
            separator = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}
