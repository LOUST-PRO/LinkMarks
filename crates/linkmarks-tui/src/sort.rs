//! Sort modes for the bookmark list.
//!
//! Kept in its own module so the [`App`](crate::app::App) can swap modes
//! without owning the comparator logic, and so unit tests can pin the
//! ordering rules without standing up the TUI.

use linkmarks_core::model::Bookmark;

/// How the bookmark list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    /// Most recent `updated_at` first (default).
    #[default]
    UpdatedDesc,
    /// Alphabetical by `title`, case-insensitive (ascending).
    TitleAsc,
    /// Alphabetical by `canonical_url`, case-insensitive (ascending).
    /// Useful for forensic dedupe review and "same domain grouping".
    CanonicalUrl,
    /// Most recent `created_at` first (newest imports on top).
    CreatedDesc,
}

impl SortMode {
    /// Return the next mode in a fixed 4-way cycle:
    /// `UpdatedDesc → TitleAsc → CanonicalUrl → CreatedDesc → UpdatedDesc`.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            SortMode::UpdatedDesc => SortMode::TitleAsc,
            SortMode::TitleAsc => SortMode::CanonicalUrl,
            SortMode::CanonicalUrl => SortMode::CreatedDesc,
            SortMode::CreatedDesc => SortMode::UpdatedDesc,
        }
    }

    /// Short status-bar label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SortMode::UpdatedDesc => "updated",
            SortMode::TitleAsc => "title",
            SortMode::CanonicalUrl => "url",
            SortMode::CreatedDesc => "created",
        }
    }

    /// All modes in cycle order. Stable across releases.
    pub const ALL: [SortMode; 4] = [
        SortMode::UpdatedDesc,
        SortMode::TitleAsc,
        SortMode::CanonicalUrl,
        SortMode::CreatedDesc,
    ];
}

/// Sort `bookmarks` in place using `mode`.
///
/// Ties are broken by `BookmarkId` so the order is fully deterministic
/// across runs.
pub fn sort_bookmarks(bookmarks: &mut [Bookmark], mode: SortMode) {
    match mode {
        SortMode::UpdatedDesc => bookmarks.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.0.cmp(&b.id.0))
        }),
        SortMode::TitleAsc => bookmarks.sort_by(|a, b| {
            a.title
                .to_lowercase()
                .cmp(&b.title.to_lowercase())
                .then_with(|| a.id.0.cmp(&b.id.0))
        }),
        SortMode::CanonicalUrl => bookmarks.sort_by(|a, b| {
            a.canonical_url
                .to_lowercase()
                .cmp(&b.canonical_url.to_lowercase())
                .then_with(|| a.id.0.cmp(&b.id.0))
        }),
        SortMode::CreatedDesc => bookmarks.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.0.cmp(&b.id.0))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use linkmarks_core::model::{BookmarkId, SourceKind, SourceRef};

    fn bm(title: &str, url: &str, created_at: i64, updated_at: i64) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: url.into(),
            canonical_url: url.into(),
            title: title.into(),
            description: None,
            tags: vec![],
            collection: None,
            created_at: Utc.timestamp_opt(created_at, 0).unwrap(),
            updated_at: Utc.timestamp_opt(updated_at, 0).unwrap(),
            source: SourceRef {
                kind: SourceKind::Chromium,
                external_id: None,
                imported_at: Utc.timestamp_opt(updated_at, 0).unwrap(),
                raw: None,
            },
            content_type: None,
            archived: false,
        }
    }

    #[test]
    fn default_is_updated_desc() {
        assert_eq!(SortMode::default(), SortMode::UpdatedDesc);
    }

    #[test]
    fn next_cycles_through_four_modes() {
        assert_eq!(SortMode::UpdatedDesc.next(), SortMode::TitleAsc);
        assert_eq!(SortMode::TitleAsc.next(), SortMode::CanonicalUrl);
        assert_eq!(SortMode::CanonicalUrl.next(), SortMode::CreatedDesc);
        assert_eq!(SortMode::CreatedDesc.next(), SortMode::UpdatedDesc);
    }

    #[test]
    fn all_constant_matches_next_cycle() {
        assert_eq!(SortMode::ALL.len(), 4);
        for (i, mode) in SortMode::ALL.iter().enumerate() {
            let next = mode.next();
            let expected = SortMode::ALL[(i + 1) % SortMode::ALL.len()];
            assert_eq!(next, expected, "ALL[{i}].next() != ALL[(i+1)%4]");
        }
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(SortMode::UpdatedDesc.label(), "updated");
        assert_eq!(SortMode::TitleAsc.label(), "title");
        assert_eq!(SortMode::CanonicalUrl.label(), "url");
        assert_eq!(SortMode::CreatedDesc.label(), "created");
    }

    #[test]
    fn empty_vec_does_not_panic() {
        let mut v: Vec<Bookmark> = vec![];
        for mode in SortMode::ALL {
            sort_bookmarks(&mut v, mode);
        }
        assert!(v.is_empty());
    }

    #[test]
    fn updated_desc_orders_newest_first() {
        let mut v = vec![
            bm("a", "https://a", 1_700_000_000, 1_700_000_000),
            bm("b", "https://b", 1_700_000_000, 1_700_000_500),
            bm("c", "https://c", 1_700_000_000, 1_700_000_100),
        ];
        sort_bookmarks(&mut v, SortMode::UpdatedDesc);
        let titles: Vec<&str> = v.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, vec!["b", "c", "a"]);
    }

    #[test]
    fn title_asc_is_case_insensitive() {
        let mut v = vec![
            bm("banana", "https://b", 1, 1),
            bm("Apple", "https://a", 1, 1),
            bm("cherry", "https://c", 1, 1),
        ];
        sort_bookmarks(&mut v, SortMode::TitleAsc);
        let titles: Vec<&str> = v.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, vec!["Apple", "banana", "cherry"]);
    }

    #[test]
    fn canonical_url_asc_is_case_insensitive() {
        let mut v = vec![
            bm("a", "https://example.com/zzz", 1, 1),
            bm("b", "https://example.com/aaa", 1, 1),
            bm("c", "https://example.com/mmm", 1, 1),
        ];
        sort_bookmarks(&mut v, SortMode::CanonicalUrl);
        let urls: Vec<&str> = v.iter().map(|b| b.canonical_url.as_str()).collect();
        assert_eq!(
            urls,
            vec![
                "https://example.com/aaa",
                "https://example.com/mmm",
                "https://example.com/zzz",
            ]
        );
    }

    #[test]
    fn created_desc_orders_newest_first() {
        let mut v = vec![
            bm("a", "https://a", 1_700_000_000, 1),
            bm("b", "https://b", 1_700_000_500, 1),
            bm("c", "https://c", 1_700_000_100, 1),
        ];
        sort_bookmarks(&mut v, SortMode::CreatedDesc);
        let titles: Vec<&str> = v.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, vec!["b", "c", "a"]);
    }

    #[test]
    fn ties_break_by_id_for_full_determinism() {
        // Same created_at + updated_at + title + url → only id breaks tie.
        let mut v = vec![
            bm("same", "https://same", 1, 1),
            bm("same", "https://same", 1, 1),
        ];
        sort_bookmarks(&mut v, SortMode::TitleAsc);
        let ids: Vec<String> = v.iter().map(|b| b.id.0.clone()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "id tie-break must be sorted ascending");
    }
}
