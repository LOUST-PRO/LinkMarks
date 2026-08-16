//! `App` — the TUI's mutable state.

use linkmarks_core::model::Bookmark;

use crate::config::AppConfig;
use crate::input::{AppAction, KeyMap};
use crate::registry::SourceRegistry;
use crate::sort::{sort_bookmarks, SortMode};
use crate::state::{AppState, FilterMode};

/// Maximum number of rows one `PageUp` / `PageDown` jumps.
const PAGE_STEP: usize = 10;

/// The TUI's mutable state.
pub struct App {
    /// Source registry: where bookmarks come from.
    pub registry: SourceRegistry,
    /// Loaded bookmarks (deduplicated, sorted).
    pub bookmarks: Vec<Bookmark>,
    /// Cursor position over `visible()`.
    pub selected: usize,
    /// State machine.
    pub state: AppState,
    /// Active sort mode. Cycled by `s` (global), applied on every
    /// `reload()` and immediately after the cycle.
    pub sort_mode: SortMode,
    /// Resolved config (kept for diagnostics + future hot-reload).
    pub config: AppConfig,
}

impl App {
    /// Build a new app. Loads bookmarks from the registry eagerly so
    /// the first frame paints something useful.
    pub fn new(registry: SourceRegistry, config: AppConfig) -> Self {
        let mut app = Self {
            registry,
            bookmarks: Vec::new(),
            selected: 0,
            state: AppState::Loading,
            sort_mode: SortMode::default(),
            config,
        };
        app.reload();
        app
    }

    /// Re-read bookmarks from the registry. Resets the cursor and
    /// re-applies the current sort mode.
    pub fn reload(&mut self) {
        let mut bookmarks = self.registry.load_all();
        sort_bookmarks(&mut bookmarks, self.sort_mode);
        self.bookmarks = bookmarks;
        self.selected = 0;
        self.state = AppState::List;
    }

    /// Apply a key event. Returns the action the loop should take.
    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) -> AppAction {
        let filter_active = self.state.is_filter();
        let action = KeyMap::map(key, filter_active);

        match action {
            AppAction::Continue => AppAction::Continue,
            AppAction::Quit(code) => {
                self.state = AppState::Quit(code);
                AppAction::Quit(code)
            }
            AppAction::ShowHelp => {
                if self.state.is_help() {
                    self.state = AppState::List;
                } else {
                    self.state = AppState::Help;
                }
                AppAction::Continue
            }
            AppAction::ShowFilter => {
                let current = match &self.state {
                    AppState::Filter { query, .. } => query.clone(),
                    _ => String::new(),
                };
                self.state = AppState::Filter {
                    query: current,
                    mode: FilterMode::Substring,
                };
                AppAction::Continue
            }
            AppAction::ExitFilter => {
                self.state = AppState::List;
                AppAction::Continue
            }
            AppAction::FilterChar(c) => {
                if let AppState::Filter { query, mode: _ } = &mut self.state {
                    query.push(c);
                    self.selected = 0;
                }
                AppAction::Continue
            }
            AppAction::FilterBackspace => {
                if let AppState::Filter { query, mode: _ } = &mut self.state {
                    query.pop();
                    self.selected = 0;
                }
                AppAction::Continue
            }
            AppAction::FilterAccept => {
                self.state = AppState::List;
                AppAction::Continue
            }
            AppAction::CycleFilterMode => {
                if let AppState::Filter { query, mode } = &mut self.state {
                    *mode = match mode {
                        FilterMode::Substring => FilterMode::Tag,
                        FilterMode::Tag => FilterMode::Fuzzy,
                        FilterMode::Fuzzy => FilterMode::Substring,
                    };
                    let _ = query; // keep the same query
                    self.selected = 0;
                }
                AppAction::Continue
            }
            AppAction::MoveUp => {
                self.move_cursor(-1);
                AppAction::Continue
            }
            AppAction::MoveDown => {
                self.move_cursor(1);
                AppAction::Continue
            }
            AppAction::PageUp => {
                self.move_cursor(-(PAGE_STEP as isize));
                AppAction::Continue
            }
            AppAction::PageDown => {
                self.move_cursor(PAGE_STEP as isize);
                AppAction::Continue
            }
            AppAction::Top => {
                self.selected = 0;
                AppAction::Continue
            }
            AppAction::Bottom => {
                let n = self.visible().len();
                if n > 0 {
                    self.selected = n - 1;
                }
                AppAction::Continue
            }
            AppAction::OpenUrl(_) => {
                if let Some(b) = self.selected_bookmark() {
                    let url = b.original_url.clone();
                    self.open_external(&url);
                    AppAction::OpenUrl(url)
                } else {
                    AppAction::Continue
                }
            }
            AppAction::Refresh => {
                self.reload();
                AppAction::Refresh
            }
            AppAction::CycleSort => {
                self.sort_mode = self.sort_mode.next();
                sort_bookmarks(&mut self.bookmarks, self.sort_mode);
                self.selected = 0;
                AppAction::Continue
            }
        }
    }

    /// Pumped by the TUI loop. Currently a no-op.
    pub fn on_tick(&mut self) -> AppAction {
        AppAction::Continue
    }

    /// The bookmarks visible under the current filter, if any.
    pub fn visible(&self) -> Vec<&Bookmark> {
        match &self.state {
            AppState::Filter { query, mode } => self
                .bookmarks
                .iter()
                .filter(|b| AppState::matches(b, query, *mode))
                .collect(),
            _ => self.bookmarks.iter().collect(),
        }
    }

    /// Currently selected bookmark (after the filter is applied).
    #[must_use]
    pub fn selected_bookmark(&self) -> Option<&Bookmark> {
        let v = self.visible();
        v.get(self.selected).copied()
    }

    fn move_cursor(&mut self, delta: isize) {
        let n = self.visible().len();
        if n == 0 {
            return;
        }
        let cur = self.selected as isize;
        let next = (cur + delta).clamp(0, (n - 1) as isize);
        self.selected = next as usize;
    }

    /// Best-effort, fire-and-forget open of `url` in the system default
    /// browser. Errors are logged at `tracing::warn!`.
    fn open_external(&self, url: &str) {
        let (cmd, arg) = if cfg!(target_os = "macos") {
            ("open", url)
        } else if cfg!(target_os = "windows") {
            tracing::warn!(
                url,
                "linkmarks-tui: open_external not supported on Windows; ignoring"
            );
            return;
        } else {
            ("xdg-open", url)
        };

        match std::process::Command::new(cmd).arg(arg).spawn() {
            Ok(mut child) => {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
            Err(e) => {
                tracing::warn!(cmd, url, error = %e, "failed to spawn browser");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SourceSelection;
    use chrono::{TimeZone, Utc};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use linkmarks_core::model::{BookmarkId, SourceKind, SourceRef};
    use std::path::PathBuf;

    fn bm(title: &str, url: &str) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: url.into(),
            canonical_url: url.into(),
            title: title.into(),
            description: None,
            tags: vec![],
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

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_f() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)
    }

    fn app_with(bookmarks: Vec<Bookmark>) -> App {
        let mut registry = SourceRegistry::resolve(
            SourceSelection::All,
            None,
            None,
            Some(PathBuf::from("/nonexistent/store.db")),
        );
        registry.sources.clear();
        let mut app = App::new(registry, AppConfig::default());
        app.bookmarks = bookmarks;
        app.selected = 0;
        app
    }

    #[test]
    fn reload_moves_to_list_state() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        app.state = AppState::Loading;
        app.reload();
        assert!(matches!(app.state, AppState::List));
    }

    #[test]
    fn cursor_movement_clamps_to_bounds() {
        let mut app = app_with(vec![
            bm("a", "https://a"),
            bm("b", "https://b"),
            bm("c", "https://c"),
        ]);
        app.on_key(key(KeyCode::Char('G')));
        assert_eq!(app.selected, 2);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 2);
        app.on_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, 0);
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn page_movement_jumps_ten() {
        let mut app = app_with(
            (0..25)
                .map(|i| bm(&format!("b{i}"), &format!("https://x/{i}")))
                .collect(),
        );
        app.on_key(key(KeyCode::PageDown));
        assert_eq!(app.selected, 10);
    }

    #[test]
    fn quit_sets_state() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        let action = app.on_key(key(KeyCode::Char('q')));
        assert_eq!(action, AppAction::Quit(0));
        assert!(matches!(app.state, AppState::Quit(0)));
    }

    #[test]
    fn filter_mode_typing_updates_visibles() {
        let mut app = app_with(vec![
            bm("Rust book", "https://doc.rust-lang.org"),
            bm("Python tutorial", "https://python.org"),
        ]);
        app.on_key(key(KeyCode::Char('/')));
        for c in "rust".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        let v = app.visible();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].title, "Rust book");
    }

    #[test]
    fn filter_esc_returns_to_full_list() {
        let mut app = app_with(vec![bm("a", "https://a"), bm("b", "https://b")]);
        app.on_key(key(KeyCode::Char('/')));
        app.on_key(key(KeyCode::Esc));
        let v = app.visible();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn open_in_filter_interpretation_does_not_move_cursor() {
        let mut app = app_with(vec![bm("a", "https://a"), bm("b", "https://b")]);
        app.on_key(key(KeyCode::Char('/')));
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn unknown_key_is_a_no_op() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        let action = app.on_key(key(KeyCode::F(12)));
        assert_eq!(action, AppAction::Continue);
    }

    // --- State-wiring integration tests (per state-gating rule) ---
    //
    // Every enum variant exposed in `state.rs` / `sort.rs` MUST have a
    // reachable keybinding and a visible App mutation exercised here.
    // The 1.x release shipped `FilterMode::Fuzzy` and a 4-variant
    // `SortMode` documented in the README with no UI path; this
    // regression class is the one we want to catch next time.

    #[test]
    fn state_wiring_sort_starts_at_default() {
        let app = app_with(vec![bm("a", "https://a")]);
        assert_eq!(app.sort_mode, SortMode::UpdatedDesc);
    }

    #[test]
    fn state_wiring_press_s_cycles_sort_mode() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        // Initial → TitleAsc
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.sort_mode, SortMode::TitleAsc);
        // TitleAsc → CanonicalUrl
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.sort_mode, SortMode::CanonicalUrl);
        // CanonicalUrl → CreatedDesc
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.sort_mode, SortMode::CreatedDesc);
        // CreatedDesc → UpdatedDesc (loop)
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.sort_mode, SortMode::UpdatedDesc);
    }

    #[test]
    fn state_wiring_press_s_actually_reorders_visible_list() {
        // Two bookmarks with predictable titles; cycle sort and verify
        // visible() reflects the new order. This pins the wiring
        // between AppAction::CycleSort → sort_bookmarks.
        let mut app = app_with(vec![
            bm("banana", "https://banana"),
            bm("apple", "https://apple"),
        ]);
        app.on_key(key(KeyCode::Char('s'))); // TitleAsc
        let titles: Vec<String> = app.visible().iter().map(|b| b.title.clone()).collect();
        assert_eq!(titles, vec!["apple", "banana"]);
    }

    #[test]
    fn state_wiring_press_s_inside_filter_is_a_no_op() {
        // `CycleSort` is global; while filter is active, `s` is
        // interpreted as a filter char. The sort mode must NOT mutate.
        let mut app = app_with(vec![bm("a", "https://a")]);
        app.on_key(key(KeyCode::Char('/')));
        for c in "ru".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        let before = app.sort_mode;
        app.on_key(key(KeyCode::Char('s')));
        assert_eq!(app.sort_mode, before);
        assert!(matches!(
            &app.state,
            AppState::Filter {
                query,
                mode: FilterMode::Substring,
            } if query == "rus"
        ));
    }

    #[test]
    fn state_wiring_filter_starts_in_substring_mode() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        app.on_key(key(KeyCode::Char('/')));
        assert!(matches!(
            app.state,
            AppState::Filter {
                mode: FilterMode::Substring,
                ..
            }
        ));
    }

    #[test]
    fn state_wiring_ctrl_f_cycles_filter_mode() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        app.on_key(key(KeyCode::Char('/')));
        // Substring → Tag
        app.on_key(ctrl_f());
        assert!(matches!(
            app.state,
            AppState::Filter {
                mode: FilterMode::Tag,
                ..
            }
        ));
        // Tag → Fuzzy
        app.on_key(ctrl_f());
        assert!(matches!(
            app.state,
            AppState::Filter {
                mode: FilterMode::Fuzzy,
                ..
            }
        ));
        // Fuzzy → Substring (loop)
        app.on_key(ctrl_f());
        assert!(matches!(
            app.state,
            AppState::Filter {
                mode: FilterMode::Substring,
                ..
            }
        ));
    }

    #[test]
    fn state_wiring_ctrl_f_preserves_query_across_cycle() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        app.on_key(key(KeyCode::Char('/')));
        for c in "ru".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(ctrl_f());
        assert!(matches!(
            &app.state,
            AppState::Filter { query, mode: FilterMode::Tag } if query == "ru"
        ));
        app.on_key(ctrl_f());
        assert!(matches!(
            &app.state,
            AppState::Filter { query, mode: FilterMode::Fuzzy } if query == "ru"
        ));
    }

    #[test]
    fn state_wiring_ctrl_f_outside_filter_is_a_no_op() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        app.on_key(ctrl_f());
        assert!(matches!(app.state, AppState::List));
    }

    #[test]
    fn state_wiring_fuzzy_mode_actually_filters() {
        // Verify the Fuzzy mode is wired to the fuzzy matcher (not a
        // silent no-op). A query that fuzzy-matches a known bookmark
        // must produce a non-empty `visible()`.
        //
        // Cycle: Substring → Tag → Fuzzy (two Ctrl+F presses from the
        // starting Substring state).
        let mut app = app_with(vec![
            bm(
                "Rust by Example",
                "https://doc.rust-lang.org/rust-by-example",
            ),
            bm("Python tutorial", "https://python.org"),
        ]);
        app.on_key(key(KeyCode::Char('/')));
        for c in "rust".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(ctrl_f()); // Substring → Tag
        app.on_key(ctrl_f()); // Tag → Fuzzy
        let v = app.visible();
        assert!(
            !v.is_empty(),
            "fuzzy 'rust' should match at least one bookmark"
        );
        // And the Fuzzy mode must be the active state.
        assert!(matches!(
            app.state,
            AppState::Filter {
                mode: FilterMode::Fuzzy,
                ..
            }
        ));
    }

    #[test]
    fn state_wiring_tag_mode_does_not_match_title() {
        // Companion to the fuzzy test: when the user cycles to Tag,
        // the title must NOT contribute to matches — only the tag list.
        // This pins the contract that the 3 FilterMode variants are
        // genuinely orthogonal.
        let mut app = app_with(vec![
            bm("hello world", "https://x.com"),
            bm("other", "https://y.com"),
        ]);
        app.on_key(key(KeyCode::Char('/')));
        for c in "hello".chars() {
            app.on_key(key(KeyCode::Char(c)));
        }
        app.on_key(ctrl_f()); // Substring → Tag
        let v = app.visible();
        // "hello" against empty tags → no matches.
        assert!(v.is_empty(), "tag mode must not match against title");
    }
}
