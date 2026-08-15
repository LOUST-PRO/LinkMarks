//! Application state — the model half of the TUI.

/// State machine for the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    /// Loading bookmarks from the registry. Briefly seen at startup.
    Loading,
    /// Browsing the list. The normal state.
    List,
    /// Help overlay on top of the list.
    Help,
    /// Filter mode: typing in the bottom bar, list filtered live.
    Filter {
        /// Current filter query (substring).
        query: String,
        /// Substring vs. tag filter mode.
        mode: FilterMode,
    },
    /// Confirm dialog. Reserved — the v1 UI never produces destructive
    /// actions, but the state lives here so future dialogs don't need
    /// a new variant.
    Confirm {
        /// The action that will fire on `y`.
        action: PendingAction,
        /// Human-readable prompt.
        prompt: String,
    },
    /// Terminal exit. Holds the exit code.
    Quit(i32),
}

/// Filter mode for the bottom bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    /// Match `query` as a case-insensitive substring against title,
    /// original_url, canonical_url, tags, and collection.
    Substring,
    /// Match only against tags.
    Tag,
    /// Fuzzy match via `nucleo`; ranked by score DESC. Empty query
    /// degenerates to "match everything".
    Fuzzy,
}

/// Pending action for a confirm dialog. Reserved for v2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingAction {
    /// Reserved placeholder.
    None,
}

impl AppState {
    /// True when the TUI should keep running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        !matches!(self, Self::Quit(_))
    }

    /// Exit code when the state is `Quit(_)`, otherwise `None`.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Quit(c) => Some(*c),
            _ => None,
        }
    }

    /// True when the help overlay is showing.
    #[must_use]
    pub fn is_help(&self) -> bool {
        matches!(self, Self::Help)
    }

    /// True when the TUI is filtering.
    #[must_use]
    pub fn is_filter(&self) -> bool {
        matches!(self, Self::Filter { .. })
    }

    /// Case-insensitive substring filter against a bookmark.
    ///
    /// When `query` is empty, every bookmark matches.
    #[must_use]
    pub fn matches(
        bookmark: &linkmarks_core::model::Bookmark,
        query: &str,
        mode: FilterMode,
    ) -> bool {
        if query.trim().is_empty() {
            return true;
        }
        let q = query.to_ascii_lowercase();
        match mode {
            FilterMode::Substring => {
                if bookmark.title.to_ascii_lowercase().contains(&q) {
                    return true;
                }
                if bookmark.original_url.to_ascii_lowercase().contains(&q) {
                    return true;
                }
                if bookmark.canonical_url.to_ascii_lowercase().contains(&q) {
                    return true;
                }
                if bookmark
                    .collection
                    .as_deref()
                    .map(|c| c.to_ascii_lowercase().contains(&q))
                    .unwrap_or(false)
                {
                    return true;
                }
                bookmark
                    .tags
                    .iter()
                    .any(|t| t.to_ascii_lowercase().contains(&q))
            }
            FilterMode::Tag => bookmark
                .tags
                .iter()
                .any(|t| t.to_ascii_lowercase().contains(&q)),
            FilterMode::Fuzzy => {
                // Delegate to the fuzzy matcher; empty query is a
                // pass-through by contract.
                !crate::filter::fuzzy_match(query, std::slice::from_ref(bookmark)).is_empty()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};

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
    fn state_is_running_until_quit() {
        assert!(AppState::Loading.is_running());
        assert!(AppState::List.is_running());
        assert!(AppState::Help.is_running());
        assert!(AppState::Filter {
            query: "x".into(),
            mode: FilterMode::Substring,
        }
        .is_running());
        assert!(!AppState::Quit(0).is_running());
    }

    #[test]
    fn exit_code_returns_only_for_quit() {
        assert_eq!(AppState::List.exit_code(), None);
        assert_eq!(AppState::Quit(0).exit_code(), Some(0));
        assert_eq!(AppState::Quit(2).exit_code(), Some(2));
    }

    #[test]
    fn matches_substring_empty_query_is_everything() {
        let b = bm("hi", "https://x.com", &["rust"]);
        assert!(AppState::matches(&b, "", FilterMode::Substring));
        assert!(AppState::matches(&b, "   ", FilterMode::Substring));
    }

    #[test]
    fn matches_substring_title_case_insensitive() {
        let b = bm("Hello World", "https://x.com", &[]);
        assert!(AppState::matches(&b, "hello", FilterMode::Substring));
        assert!(AppState::matches(&b, "WORLD", FilterMode::Substring));
        assert!(!AppState::matches(&b, "missing", FilterMode::Substring));
    }

    #[test]
    fn matches_substring_url_and_tags() {
        let b = bm("t", "https://example.com/foo", &["rust", "cli"]);
        assert!(AppState::matches(&b, "example.com", FilterMode::Substring));
        assert!(AppState::matches(&b, "rust", FilterMode::Substring));
        assert!(AppState::matches(&b, "CLI", FilterMode::Substring));
    }

    #[test]
    fn matches_tag_mode_only_tags() {
        let b = bm("hello world", "https://x.com", &["rust"]);
        assert!(AppState::matches(&b, "rust", FilterMode::Tag));
        assert!(!AppState::matches(&b, "hello", FilterMode::Tag));
    }

    #[test]
    fn filter_state_is_filter() {
        let s = AppState::Filter {
            query: "x".into(),
            mode: FilterMode::Substring,
        };
        assert!(s.is_filter());
    }

    #[test]
    fn help_state_is_help() {
        assert!(AppState::Help.is_help());
        assert!(!AppState::List.is_help());
    }
}
