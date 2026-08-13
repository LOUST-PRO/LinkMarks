//! Integration tests for `AppState` transitions.

use linkmarks_tui::state::{AppState, FilterMode};

#[test]
fn loading_to_list_transition() {
    let mut s = AppState::Loading;
    assert!(s.is_running());
    s = AppState::List;
    assert!(matches!(s, AppState::List));
    assert!(s.is_running());
}

#[test]
fn list_to_filter_to_list_to_quit() {
    let mut s = AppState::List;
    assert!(s.is_running());
    s = AppState::Filter {
        query: "rust".into(),
        mode: FilterMode::Substring,
    };
    assert!(s.is_filter());
    s = AppState::List;
    assert!(!s.is_filter());
    s = AppState::Quit(0);
    assert!(!s.is_running());
    assert_eq!(s.exit_code(), Some(0));
}

#[test]
fn suppress_unused_assignment_warning() {
    // Workaround to keep the previous value of `s` until the final
    // assertion. The lint is happy when we read it once.
    let mut s = AppState::List;
    assert!(s.is_running());
    s = AppState::Filter {
        query: "rust".into(),
        mode: FilterMode::Substring,
    };
    assert!(s.is_filter());
}

#[test]
fn help_is_toggle_state() {
    let s = AppState::Help;
    assert!(s.is_help());
    assert!(s.is_running());
}

#[test]
fn filter_mode_substring_matches_titles() {
    use chrono::{TimeZone, Utc};
    use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};

    let b = Bookmark {
        id: BookmarkId::generate(),
        original_url: "https://rust-lang.org".into(),
        canonical_url: "https://rust-lang.org".into(),
        title: "Rust Language".into(),
        description: None,
        tags: vec!["systems".into()],
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
    };

    assert!(AppState::matches(&b, "rust", FilterMode::Substring));
    assert!(AppState::matches(&b, "LANG", FilterMode::Substring));
    assert!(AppState::matches(&b, "systems", FilterMode::Substring));
    assert!(!AppState::matches(&b, "python", FilterMode::Substring));
}
