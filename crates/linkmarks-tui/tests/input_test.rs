//! Integration tests for `KeyMap`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use linkmarks_tui::input::{AppAction, KeyMap};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn jk_gl_motion_works_outside_filter() {
    assert_eq!(KeyMap::map(key(KeyCode::Char('j')), false), AppAction::MoveDown);
    assert_eq!(KeyMap::map(key(KeyCode::Char('k')), false), AppAction::MoveUp);
    assert_eq!(KeyMap::map(key(KeyCode::Char('g')), false), AppAction::Top);
    assert_eq!(KeyMap::map(key(KeyCode::Char('G')), false), AppAction::Bottom);
}

#[test]
fn arrow_keys_motion_outside_filter() {
    assert_eq!(KeyMap::map(key(KeyCode::Down), false), AppAction::MoveDown);
    assert_eq!(KeyMap::map(key(KeyCode::Up), false), AppAction::MoveUp);
    assert_eq!(KeyMap::map(key(KeyCode::PageUp), false), AppAction::PageUp);
    assert_eq!(KeyMap::map(key(KeyCode::PageDown), false), AppAction::PageDown);
}

#[test]
fn enter_emits_open_url() {
    match KeyMap::map(key(KeyCode::Enter), false) {
        AppAction::OpenUrl(_) => {}
        other => panic!("expected OpenUrl, got {:?}", other),
    }
}

#[test]
fn slash_enters_filter() {
    assert_eq!(KeyMap::map(key(KeyCode::Char('/')), false), AppAction::ShowFilter);
}

#[test]
fn question_mark_shows_help() {
    assert_eq!(KeyMap::map(key(KeyCode::Char('?')), false), AppAction::ShowHelp);
}

#[test]
fn q_quits_outside_filter() {
    assert_eq!(KeyMap::map(key(KeyCode::Char('q')), false), AppAction::Quit(0));
}

#[test]
fn q_in_filter_is_a_character() {
    assert_eq!(
        KeyMap::map(key(KeyCode::Char('q')), true),
        AppAction::FilterChar('q')
    );
}

#[test]
fn esc_in_filter_exits_it() {
    assert_eq!(KeyMap::map(key(KeyCode::Esc), true), AppAction::ExitFilter);
}

#[test]
fn enter_in_filter_accepts() {
    assert_eq!(
        KeyMap::map(key(KeyCode::Enter), true),
        AppAction::FilterAccept
    );
}

#[test]
fn backspace_in_filter_pops() {
    assert_eq!(
        KeyMap::map(key(KeyCode::Backspace), true),
        AppAction::FilterBackspace
    );
}

#[test]
fn refresh_bound_to_r() {
    assert_eq!(KeyMap::map(key(KeyCode::Char('r')), false), AppAction::Refresh);
}
