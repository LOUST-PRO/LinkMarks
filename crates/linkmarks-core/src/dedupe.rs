//! Local deterministic dedupe by canonical URL.
//!
//! Algorithm:
//! 1. Group by `canonical_url`.
//! 2. Within each group, pick the canonical record (oldest
//!    `created_at`, ties broken by lowest `id`).
//! 3. Report all conflicts: same canonical URL but differing title /
//!    tags / collection.
//!
//! Per SPEC.md §Feature 5, the CLI exposes `--dry-run` (default) and
//! `--apply`. Apply is gated on an explicit token at the CLI layer.
//! This crate provides the algorithm; the CLI provides the gate.

use crate::model::Bookmark;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single conflict record. Two bookmarks share `canonical_url` but
/// disagree on a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictRecord {
    /// The shared canonical URL.
    pub canonical_url: String,
    /// The chosen canonical record (oldest `created_at`, lowest id).
    pub chosen_id: String,
    /// The IDs of the conflicting records (excluding the chosen one).
    pub conflicting_ids: Vec<String>,
    /// Field names where the conflicting records disagree with the
    /// chosen record.
    pub differing_fields: Vec<String>,
}

/// Full dedupe report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DedupeReport {
    /// Number of distinct canonical URLs after dedupe.
    pub canonical_count: usize,
    /// Number of records that were merged into a canonical record.
    pub merged_count: usize,
    /// Conflicts found (canonical URL with disagreement).
    pub conflicts: Vec<ConflictRecord>,
}

/// Run the dedupe algorithm.
///
/// `bookmarks` is the input set (any order). The output report is
/// deterministic: the same input produces the same report bytes across
/// runs.
///
/// Returned `Vec<Bookmark>` is the canonical set (one record per
/// canonical URL).
pub fn dedupe(bookmarks: &[Bookmark]) -> (Vec<Bookmark>, DedupeReport) {
    // Group by canonical URL using BTreeMap for stable iteration.
    let mut groups: BTreeMap<String, Vec<&Bookmark>> = BTreeMap::new();
    for b in bookmarks {
        groups.entry(b.canonical_url.clone()).or_default().push(b);
    }

    let mut canonical: Vec<Bookmark> = Vec::with_capacity(groups.len());
    let mut conflicts: Vec<ConflictRecord> = Vec::new();
    let mut merged = 0usize;

    for (canonical_url, group) in &groups {
        // Pick canonical: oldest created_at, tie-break lowest id.
        let chosen = group
            .iter()
            .min_by(|a, b| {
                a.created_at
                    .cmp(&b.created_at)
                    .then_with(|| a.id.0.cmp(&b.id.0))
            })
            .expect("non-empty group");

        // Detect conflicts.
        let mut differing: Vec<String> = Vec::new();
        let mut conflicting_ids: Vec<String> = Vec::new();
        for b in group {
            if b.id == chosen.id {
                continue;
            }
            let mut differs = false;
            if b.title != chosen.title {
                if !differing.iter().any(|f| f == "title") {
                    differing.push("title".to_string());
                }
                differs = true;
            }
            if b.tags != chosen.tags {
                if !differing.iter().any(|f| f == "tags") {
                    differing.push("tags".to_string());
                }
                differs = true;
            }
            if b.collection != chosen.collection {
                if !differing.iter().any(|f| f == "collection") {
                    differing.push("collection".to_string());
                }
                differs = true;
            }
            if differs {
                conflicting_ids.push(b.id.0.clone());
            }
        }
        if !conflicting_ids.is_empty() {
            conflicts.push(ConflictRecord {
                canonical_url: canonical_url.clone(),
                chosen_id: chosen.id.0.clone(),
                conflicting_ids,
                differing_fields: differing,
            });
        }

        merged += group.len() - 1;
        canonical.push((*chosen).clone());
    }

    let report = DedupeReport {
        canonical_count: canonical.len(),
        merged_count: merged,
        conflicts,
    };

    (canonical, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BookmarkId, SourceKind, SourceRef};
    use chrono::{TimeZone, Utc};

    fn mk(id: &str, canonical: &str, title: &str, created_secs: i64) -> Bookmark {
        Bookmark {
            id: BookmarkId(id.into()),
            original_url: format!("https://example.com/{id}"),
            canonical_url: canonical.into(),
            title: title.into(),
            description: None,
            tags: vec![],
            collection: None,
            created_at: Utc.timestamp_opt(created_secs, 0).unwrap(),
            updated_at: Utc.timestamp_opt(created_secs, 0).unwrap(),
            source: SourceRef {
                kind: SourceKind::Manual,
                external_id: None,
                imported_at: Utc.timestamp_opt(0, 0).unwrap(),
                raw: None,
            },
            content_type: None,
            archived: false,
        }
    }

    #[test]
    fn dedupes_by_canonical_url() {
        let bs = vec![
            mk("a", "https://example.com/x", "Title A", 100),
            mk("b", "https://example.com/x", "Title A", 200),
            mk("c", "https://example.com/y", "Title C", 150),
        ];
        let (canon, report) = dedupe(&bs);
        assert_eq!(canon.len(), 2);
        assert_eq!(report.merged_count, 1);
        assert_eq!(report.canonical_count, 2);
        assert!(report.conflicts.is_empty());
    }

    #[test]
    fn picks_oldest_as_canonical() {
        let bs = vec![
            mk("a", "https://example.com/x", "Newer", 200),
            mk("b", "https://example.com/x", "Older", 100),
        ];
        let (canon, _) = dedupe(&bs);
        assert_eq!(canon.len(), 1);
        assert_eq!(canon[0].id.0, "b");
    }

    #[test]
    fn tie_break_by_id() {
        let bs = vec![
            mk("zzz", "https://example.com/x", "Same time", 100),
            mk("aaa", "https://example.com/x", "Same time", 100),
        ];
        let (canon, _) = dedupe(&bs);
        assert_eq!(canon[0].id.0, "aaa");
    }

    #[test]
    fn reports_conflict_on_title_mismatch() {
        let bs = vec![
            mk("a", "https://example.com/x", "Title A", 100),
            mk("b", "https://example.com/x", "Title B", 200),
        ];
        let (_, report) = dedupe(&bs);
        assert_eq!(report.conflicts.len(), 1);
        assert!(report.conflicts[0]
            .differing_fields
            .iter()
            .any(|f| f == "title"));
    }

    #[test]
    fn report_is_deterministic() {
        let bs = vec![
            mk("a", "https://example.com/x", "A", 100),
            mk("b", "https://example.com/x", "B", 200),
            mk("c", "https://example.com/y", "C", 150),
        ];
        let (_, r1) = dedupe(&bs);
        let (_, r2) = dedupe(&bs);
        let s1 = serde_json::to_string(&r1).unwrap();
        let s2 = serde_json::to_string(&r2).unwrap();
        assert_eq!(s1, s2);
    }
}
