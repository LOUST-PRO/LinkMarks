//! Domain model for LinkMarks.
//!
//! Invariants (per `docs/ARCHITECTURE.md`):
//! - `original_url` is **never** rewritten. Round-trip fidelity.
//! - `canonical_url` is the dedupe key, normalized per ADR-0001.
//! - `tags` are sorted, lowercase, deduplicated at the model boundary.
//! - `collection` is a `/`-separated folder path, normalized.
//! - Timestamps are UTC ISO 8601.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Opaque bookmark identifier. ULID by default; may be locally generated
/// UUIDs in v0. Server-assigned in CRDT mode (Fase 3+).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BookmarkId(pub String);

impl BookmarkId {
    /// Generate a fresh ULID-backed identifier.
    #[must_use]
    pub fn generate() -> Self {
        Self(ulid::Ulid::new().to_string())
    }
}

impl std::fmt::Display for BookmarkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque collection identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CollectionId(pub String);

/// A normalized bookmark record.
///
/// `original_url` is preserved verbatim from the source. `canonical_url`
/// is normalized for dedupe (see `canonical::canonicalize`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    /// Stable identifier.
    pub id: BookmarkId,
    /// Raw URL as imported (never rewritten).
    pub original_url: String,
    /// Normalized URL; the dedupe key.
    pub canonical_url: String,
    /// Display title, trimmed.
    pub title: String,
    /// Optional human-readable description.
    pub description: Option<String>,
    /// Sorted, lowercase, deduplicated tags.
    pub tags: Vec<String>,
    /// Folder path, `/`-separated.
    pub collection: Option<String>,
    /// First-seen timestamp (source clock or import time).
    pub created_at: DateTime<Utc>,
    /// Last-modified timestamp.
    pub updated_at: DateTime<Utc>,
    /// Provenance — where this record came from.
    pub source: SourceRef,
    /// Sniffed or declared MIME type, if known.
    pub content_type: Option<String>,
    /// Soft-delete marker (preserves history).
    pub archived: bool,
}

/// Source provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// Kind of source.
    pub kind: SourceKind,
    /// External (provider-side) identifier, if any.
    pub external_id: Option<String>,
    /// When this record was imported into LinkMarks.
    pub imported_at: DateTime<Utc>,
    /// Original payload for audit. Bridges may populate; CLI does
    /// not require it.
    pub raw: Option<serde_json::Value>,
}

/// Enumerates the source kinds supported by core + bridges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// Chromium-family browser JSON (Chrome, Brave, Edge, Arc, Vivaldi, Opera).
    Chromium,
    /// Firefox places.sqlite + jsonlz4 backups (Fase 2).
    Firefox,
    /// Netscape HTML interchange format.
    Netscape,
    /// Pinboard REST API (Fase 2+).
    Pinboard,
    /// Linkwarden REST API (Fase 2+).
    Linkwarden,
    /// Manually entered by a user.
    Manual,
}

impl SourceKind {
    /// Lowercase identifier used on the CLI (`--source=chrome`).
    #[must_use]
    pub fn as_cli_str(&self) -> &'static str {
        match self {
            Self::Chromium => "chrome",
            Self::Firefox => "firefox",
            Self::Netscape => "netscape",
            Self::Pinboard => "pinboard",
            Self::Linkwarden => "linkwarden",
            Self::Manual => "manual",
        }
    }

    /// Parse from the CLI flag value.
    pub fn from_cli_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "chrome" | "chromium" | "brave" | "edge" | "arc" | "vivaldi" | "opera" => {
                Some(Self::Chromium)
            }
            "firefox" => Some(Self::Firefox),
            "netscape" | "html" => Some(Self::Netscape),
            "pinboard" => Some(Self::Pinboard),
            "linkwarden" => Some(Self::Linkwarden),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// A collection (folder) grouping bookmarks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// Stable identifier.
    pub id: CollectionId,
    /// Human-readable name.
    pub name: String,
    /// Parent collection, if nested.
    pub parent: Option<CollectionId>,
    /// Source kind that produced this collection.
    pub source: SourceKind,
}

/// Tag newtype. Validates lowercase + non-empty on construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tag(pub String);

impl Tag {
    /// Construct a tag, normalizing to lowercase and trimming
    /// whitespace. Returns `None` for empty input.
    #[must_use]
    pub fn new(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            None
        } else {
            Some(Self(trimmed))
        }
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_kind_cli_round_trip() {
        for kind in [
            SourceKind::Chromium,
            SourceKind::Firefox,
            SourceKind::Netscape,
            SourceKind::Pinboard,
            SourceKind::Linkwarden,
            SourceKind::Manual,
        ] {
            let s = kind.as_cli_str();
            let back = SourceKind::from_cli_str(s).expect("round-trip");
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn source_kind_accepts_browser_aliases() {
        for alias in [
            "chrome", "chromium", "brave", "edge", "arc", "vivaldi", "opera",
        ] {
            assert_eq!(SourceKind::from_cli_str(alias), Some(SourceKind::Chromium));
        }
    }

    #[test]
    fn source_kind_rejects_unknown() {
        assert_eq!(SourceKind::from_cli_str("bogus"), None);
    }

    #[test]
    fn tag_normalizes_lowercase_and_trim() {
        let tag = Tag::new("  Rust  ").unwrap();
        assert_eq!(tag.0, "rust");
    }

    #[test]
    fn tag_rejects_empty() {
        assert!(Tag::new("   ").is_none());
        assert!(Tag::new("").is_none());
    }

    #[test]
    fn bookmark_id_generates_unique() {
        let a = BookmarkId::generate();
        let b = BookmarkId::generate();
        assert_ne!(a, b);
        // ULID is 26 chars
        assert_eq!(a.0.len(), 26);
    }
}
