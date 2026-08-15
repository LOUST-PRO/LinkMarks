//! Key bindings. Maps `KeyEvent` to `AppAction`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// What the TUI loop should do in response to a key (or a tick).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    /// No state change. The loop continues.
    Continue,
    /// Quit gracefully with the given exit code (0 = clean).
    Quit(i32),
    /// The user pressed Enter on a bookmark; the URL should be opened
    /// in the system browser.
    OpenUrl(String),
    /// Re-read bookmarks from the registry.
    Refresh,
    /// Show the help overlay.
    ShowHelp,
    /// Enter filter mode.
    ShowFilter,
    /// Exit filter mode (Esc) but keep the current filtered view.
    ExitFilter,
    /// Append a character to the filter query.
    FilterChar(char),
    /// Pop the last character from the filter query.
    FilterBackspace,
    /// Accept the current filter (Enter inside filter mode).
    FilterAccept,
    /// Move the cursor up by one.
    MoveUp,
    /// Move the cursor down by one.
    MoveDown,
    /// Page up (10 rows).
    PageUp,
    /// Page down (10 rows).
    PageDown,
    /// Jump to the first row.
    Top,
    /// Jump to the last row.
    Bottom,
}

/// Default key bindings.
///
/// Independent of the `App`: this is a pure mapping so the tests can
/// pin behavior without standing up a full TUI.
#[derive(Debug, Clone, Copy)]
pub struct KeyMap;

impl KeyMap {
    /// Map a key event to an action.
    ///
    /// `filter_active` toggles the mode: when the TUI is in `Filter`,
    /// the same key press is interpreted differently (e.g. `j` is a
    /// character, not a cursor move).
    #[must_use]
    pub fn map(key: KeyEvent, filter_active: bool) -> AppAction {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return AppAction::Quit(0);
        }

        if filter_active {
            return map_in_filter(key);
        }

        map_global(key)
    }
}

fn map_global(key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => AppAction::Quit(0),
        KeyCode::Char('j') | KeyCode::Down => AppAction::MoveDown,
        KeyCode::Char('k') | KeyCode::Up => AppAction::MoveUp,
        KeyCode::Char('g') => AppAction::Top,
        KeyCode::Char('G') => AppAction::Bottom,
        KeyCode::PageUp => AppAction::PageUp,
        KeyCode::PageDown => AppAction::PageDown,
        KeyCode::Enter => AppAction::OpenUrl(String::new()),
        KeyCode::Char('/') => AppAction::ShowFilter,
        KeyCode::Char('?') => AppAction::ShowHelp,
        KeyCode::F(1) => AppAction::ShowHelp,
        KeyCode::Char('r') | KeyCode::F(5) => AppAction::Refresh,
        KeyCode::Esc => AppAction::ShowHelp,
        _ => AppAction::Continue,
    }
}

fn map_in_filter(key: KeyEvent) -> AppAction {
    match key.code {
        KeyCode::Esc => AppAction::ExitFilter,
        KeyCode::Enter => AppAction::FilterAccept,
        KeyCode::Backspace => AppAction::FilterBackspace,
        KeyCode::Char(c) => AppAction::FilterChar(c),
        _ => AppAction::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_c() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
    }

    #[test]
    fn quit_on_q() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('q')), false),
            AppAction::Quit(0)
        );
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('Q')), false),
            AppAction::Quit(0)
        );
    }

    #[test]
    fn quit_on_ctrl_c_even_in_filter() {
        assert_eq!(KeyMap::map(ctrl_c(), false), AppAction::Quit(0));
        assert_eq!(KeyMap::map(ctrl_c(), true), AppAction::Quit(0));
    }

    #[test]
    fn cursor_motion_vim_and_arrows() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('j')), false),
            AppAction::MoveDown
        );
        assert_eq!(KeyMap::map(key(KeyCode::Down), false), AppAction::MoveDown);
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('k')), false),
            AppAction::MoveUp
        );
        assert_eq!(KeyMap::map(key(KeyCode::Up), false), AppAction::MoveUp);
        assert_eq!(KeyMap::map(key(KeyCode::Char('g')), false), AppAction::Top);
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('G')), false),
            AppAction::Bottom
        );
    }

    #[test]
    fn page_motion() {
        assert_eq!(KeyMap::map(key(KeyCode::PageUp), false), AppAction::PageUp);
        assert_eq!(
            KeyMap::map(key(KeyCode::PageDown), false),
            AppAction::PageDown
        );
    }

    #[test]
    fn open_on_enter() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Enter), false),
            AppAction::OpenUrl(String::new())
        );
    }

    #[test]
    fn filter_and_help() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('/')), false),
            AppAction::ShowFilter
        );
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('?')), false),
            AppAction::ShowHelp
        );
        assert_eq!(KeyMap::map(key(KeyCode::F(1)), false), AppAction::ShowHelp);
    }

    #[test]
    fn refresh_on_r_or_f5() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('r')), false),
            AppAction::Refresh
        );
        assert_eq!(KeyMap::map(key(KeyCode::F(5)), false), AppAction::Refresh);
    }

    #[test]
    fn filter_mode_uses_chars() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('j')), true),
            AppAction::FilterChar('j')
        );
        assert_eq!(
            KeyMap::map(key(KeyCode::Char('k')), true),
            AppAction::FilterChar('k')
        );
    }

    #[test]
    fn filter_mode_esc_exits() {
        assert_eq!(KeyMap::map(key(KeyCode::Esc), true), AppAction::ExitFilter);
    }

    #[test]
    fn filter_mode_enter_accepts() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Enter), true),
            AppAction::FilterAccept
        );
    }

    #[test]
    fn filter_mode_backspace_pops() {
        assert_eq!(
            KeyMap::map(key(KeyCode::Backspace), true),
            AppAction::FilterBackspace
        );
    }

    #[test]
    fn filter_mode_arrows_are_no_ops() {
        assert_eq!(KeyMap::map(key(KeyCode::Down), true), AppAction::Continue);
        assert_eq!(KeyMap::map(key(KeyCode::Up), true), AppAction::Continue);
    }

    #[test]
    fn unknown_key_is_continue() {
        assert_eq!(KeyMap::map(key(KeyCode::F(12)), false), AppAction::Continue);
    }
}
