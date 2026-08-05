//! `App` — the TUI's mutable state.

use linkmarks_core::model::Bookmark;

use crate::config::AppConfig;
use crate::input::{AppAction, KeyMap};
use crate::registry::SourceRegistry;
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
            config,
        };
        app.reload();
        app
    }

    /// Re-read bookmarks from the registry. Resets the cursor.
    pub fn reload(&mut self) {
        let bookmarks = self.registry.load_all();
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
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('G'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.selected, 2);
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.selected, 2);
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('g'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.selected, 0);
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('k'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn page_movement_jumps_ten() {
        let mut app = app_with(
            (0..25)
                .map(|i| bm(&format!("b{i}"), &format!("https://x/{i}")))
                .collect(),
        );
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::PageDown,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.selected, 10);
    }

    #[test]
    fn quit_sets_state() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        let action = app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('q'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, AppAction::Quit(0));
        assert!(matches!(app.state, AppState::Quit(0)));
    }

    #[test]
    fn filter_mode_typing_updates_visibles() {
        let mut app = app_with(vec![
            bm("Rust book", "https://doc.rust-lang.org"),
            bm("Python tutorial", "https://python.org"),
        ]);
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('/'),
            crossterm::event::KeyModifiers::NONE,
        ));
        for c in "rust".chars() {
            app.on_key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char(c),
                crossterm::event::KeyModifiers::NONE,
            ));
        }
        let v = app.visible();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].title, "Rust book");
    }

    #[test]
    fn filter_esc_returns_to_full_list() {
        let mut app = app_with(vec![bm("a", "https://a"), bm("b", "https://b")]);
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('/'),
            crossterm::event::KeyModifiers::NONE,
        ));
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        let v = app.visible();
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn open_in_filter_interpretation_does_not_move_cursor() {
        let mut app = app_with(vec![bm("a", "https://a"), bm("b", "https://b")]);
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('/'),
            crossterm::event::KeyModifiers::NONE,
        ));
        app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn unknown_key_is_a_no_op() {
        let mut app = app_with(vec![bm("a", "https://a")]);
        let action = app.on_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::F(12),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, AppAction::Continue);
    }
}
