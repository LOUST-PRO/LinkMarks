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
    /// Group by source kind, then alphabetical by title.
    SourceAsc,
}

impl SortMode {
    /// Return the next mode in a fixed 3-way cycle:
    /// `UpdatedDesc → TitleAsc → SourceAsc → UpdatedDesc`.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            SortMode::UpdatedDesc => SortMode::TitleAsc,
            SortMode::TitleAsc => SortMode::SourceAsc,
            SortMode::SourceAsc => SortMode::UpdatedDesc,
        }
    }

    /// Short status-bar label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            SortMode::UpdatedDesc => "updated",
            SortMode::TitleAsc => "title",
            SortMode::SourceAsc => "source",
        }
    }
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
        SortMode::SourceAsc => bookmarks.sort_by(|a, b| {
            a.source
                .kind
                .as_cli_str()
                .cmp(b.source.kind.as_cli_str())
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                .then_with(|| a.id.0.cmp(&b.id.0))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use linkmarks_core::model::{BookmarkId, SourceKind, SourceRef};

    fn bm(title: &str, kind: SourceKind, updated_at: i64) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: format!("https://example.com/{title}"),
            canonical_url: format!("https://example.com/{title}"),
            title: title.into(),
            description: None,
            tags: vec![],
            collection: None,
            created_at: Utc.timestamp_opt(updated_at, 0).unwrap(),
            updated_at: Utc.timestamp_opt(updated_at, 0).unwrap(),
            source: SourceRef {
                kind,
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
    fn next_cycles_through_three_modes() {
        assert_eq!(SortMode::UpdatedDesc.next(), SortMode::TitleAsc);
        assert_eq!(SortMode::TitleAsc.next(), SortMode::SourceAsc);
        assert_eq!(SortMode::SourceAsc.next(), SortMode::UpdatedDesc);
    }

    #[test]
    fn labels_are_stable() {
        assert_eq!(SortMode::UpdatedDesc.label(), "updated");
        assert_eq!(SortMode::TitleAsc.label(), "title");
        assert_eq!(SortMode::SourceAsc.label(), "source");
    }

    #[test]
    fn empty_vec_does_not_panic() {
        let mut v: Vec<Bookmark> = vec![];
        for mode in [
            SortMode::UpdatedDesc,
            SortMode::TitleAsc,
            SortMode::SourceAsc,
        ] {
            sort_bookmarks(&mut v, mode);
        }
        assert!(v.is_empty());
    }

    #[test]
    fn updated_desc_orders_newest_first() {
        let mut v = vec![
            bm("a", SourceKind::Chromium, 1_700_000_000),
            bm("b", SourceKind::Chromium, 1_700_000_500),
            bm("c", SourceKind::Chromium, 1_700_000_100),
        ];
        sort_bookmarks(&mut v, SortMode::UpdatedDesc);
        let titles: Vec<&str> = v.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, vec!["b", "c", "a"]);
    }

    #[test]
    fn title_asc_is_case_insensitive() {
        let mut v = vec![
            bm("banana", SourceKind::Chromium, 1),
            bm("Apple", SourceKind::Chromium, 1),
            bm("cherry", SourceKind::Chromium, 1),
        ];
        sort_bookmarks(&mut v, SortMode::TitleAsc);
        let titles: Vec<&str> = v.iter().map(|b| b.title.as_str()).collect();
        // "Apple" → "apple"; at index 2 "apple" < "apricot" because 'p' < 'r'.
        assert_eq!(titles, vec!["Apple", "banana", "cherry"]);
    }

    #[test]
    fn source_asc_groups_by_source_then_title() {
        let mut v = vec![
            bm("netscape-z", SourceKind::Netscape, 1),
            bm("chrome-a", SourceKind::Chromium, 1),
            bm("chrome-b", SourceKind::Chromium, 1),
            bm("firefox-z", SourceKind::Firefox, 1),
        ];
        sort_bookmarks(&mut v, SortMode::SourceAsc);
        let labels: Vec<String> = v
            .iter()
            .map(|b| format!("{}/{}", b.source.kind.as_cli_str(), b.title))
            .collect();
        assert_eq!(
            labels,
            vec![
                "chrome/chrome-a".to_string(),
                "chrome/chrome-b".to_string(),
                "firefox/firefox-z".to_string(),
                "netscape/netscape-z".to_string(),
            ]
        );
    }
}
