//! Fuzzy filter for the TUI list.
//!
//! Wraps the [`nucleo`](https://docs.rs/nucleo) matcher so the
//! rest of the crate does not depend on its API surface. Results
//! are returned as `(index, score)` pairs sorted by score DESC.
//!
//! Empty / whitespace queries return every bookmark with score 0
//! (deterministic, original-order) — the App treats that as "no
//! filter".

use linkmarks_core::model::Bookmark;
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};

/// Run a fuzzy match of `query` against `bookmarks`.
///
/// The returned `Vec` is sorted by score descending; ties keep the
/// original input order (stable sort). An empty query (or a
/// whitespace-only one) returns every index with score `0` so the
/// caller can treat the result as a pass-through.
#[must_use]
pub fn fuzzy_match(query: &str, bookmarks: &[Bookmark]) -> Vec<(usize, u32)> {
    let mut matcher = build_matcher();
    let pattern = build_pattern(query);

    // Empty / whitespace query: pass-through, original order, score 0.
    if pattern.is_none() {
        return (0..bookmarks.len()).map(|idx| (idx, 0)).collect();
    }

    let mut out: Vec<(usize, u32)> = bookmarks
        .iter()
        .enumerate()
        .filter_map(|(idx, b)| {
            let haystack = haystack_for(b);
            score_one(&mut matcher, &pattern, &haystack).map(|s| (idx, s))
        })
        .collect();
    // Defensive: nucleo usually returns matches in input order, but
    // for empty patterns we want that order preserved. For scored
    // results we sort by score DESC with stable tie-breaking.
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

fn build_matcher() -> Matcher {
    Matcher::new(Config::DEFAULT)
}

fn build_pattern(query: &str) -> Option<Pattern> {
    if query.trim().is_empty() {
        return None;
    }
    // `Ignore` (case-insensitive) for bookmark search is the right
    // default: users typing "HELLO" almost always mean "hello".
    // `Smart` (lowercase-only = lenient, any-uppercase = strict) is
    // a worse UX for this domain — a bookmark titled "Rust by
    // Example" would silently not match a query of "RUST".
    Some(Pattern::parse(
        query,
        CaseMatching::Ignore,
        Normalization::Smart,
    ))
}

fn score_one(matcher: &mut Matcher, pattern: &Option<Pattern>, haystack: &str) -> Option<u32> {
    let pattern = pattern.as_ref()?;
    let mut buf = Vec::with_capacity(haystack.len());
    let haystack = Utf32Str::new(haystack, &mut buf);
    pattern.score(haystack, matcher)
}

/// Build the searchable text for a bookmark.
///
/// Concatenates `title`, `original_url`, `canonical_url`, optional
/// `description`, tags, and optional `collection` with single-space
/// separators. Empty fields are skipped so the haystack stays
/// compact (avoids matching " " against an empty field).
#[must_use]
pub fn haystack_for(b: &Bookmark) -> String {
    let mut out =
        String::with_capacity(b.title.len() + b.original_url.len() + b.canonical_url.len() + 32);
    push_if_nonempty(&mut out, &b.title);
    push_if_nonempty(&mut out, &b.original_url);
    push_if_nonempty(&mut out, &b.canonical_url);
    if let Some(desc) = &b.description {
        push_if_nonempty(&mut out, desc);
    }
    if let Some(coll) = &b.collection {
        push_if_nonempty(&mut out, coll);
    }
    for tag in &b.tags {
        push_if_nonempty(&mut out, tag);
    }
    out
}

fn push_if_nonempty(out: &mut String, field: &str) {
    let trimmed = field.trim();
    if trimmed.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(trimmed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use linkmarks_core::model::{BookmarkId, SourceKind, SourceRef};

    fn bm(title: &str, url: &str, tags: &[&str]) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: url.into(),
            canonical_url: url.into(),
            title: title.into(),
            description: None,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            collection: None,
            created_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            updated_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            source: SourceRef {
                kind: SourceKind::Chromium,
                external_id: None,
                imported_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
                raw: None,
            },
            content_type: None,
            archived: false,
        }
    }

    #[test]
    fn empty_query_returns_all_with_zero_score() {
        let list = vec![bm("a", "https://a", &[]), bm("b", "https://b", &[])];
        let r = fuzzy_match("", &list);
        assert_eq!(r.len(), 2);
        assert!(r.iter().all(|(_, s)| *s == 0));
        assert_eq!(r[0].0, 0);
        assert_eq!(r[1].0, 1);
    }

    #[test]
    fn whitespace_query_returns_all_with_zero_score() {
        let list = vec![bm("a", "https://a", &[])];
        let r = fuzzy_match("   ", &list);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, 0);
    }

    #[test]
    fn haystack_for_combines_fields() {
        let mut b = bm("Title", "https://example.com", &["rust", "cli"]);
        b.description = Some("Some desc".into());
        b.collection = Some("work".into());
        let h = haystack_for(&b);
        assert!(h.contains("Title"));
        assert!(h.contains("https://example.com"));
        assert!(h.contains("Some desc"));
        assert!(h.contains("work"));
        assert!(h.contains("rust"));
        assert!(h.contains("cli"));
    }

    #[test]
    fn haystack_for_skips_empty_fields() {
        let mut b = bm("Title", "https://x", &[]);
        b.collection = Some("   ".into());
        b.description = Some(String::new());
        let h = haystack_for(&b);
        assert!(!h.contains("  "));
    }

    #[test]
    fn push_if_nonempty_handles_whitespace_only() {
        let mut s = String::new();
        push_if_nonempty(&mut s, "   ");
        assert!(s.is_empty());
        push_if_nonempty(&mut s, "hello");
        assert_eq!(s, "hello");
        push_if_nonempty(&mut s, "");
        assert_eq!(s, "hello");
        push_if_nonempty(&mut s, "world");
        assert_eq!(s, "hello world");
    }
}
