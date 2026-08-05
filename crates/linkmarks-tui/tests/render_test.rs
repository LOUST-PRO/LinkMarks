//! Renderer integration tests using `ratatui::backend::TestBackend`.

use chrono::{TimeZone, Utc};
use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};
use linkmarks_tui::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn bm(title: &str, url: &str) -> Bookmark {
    Bookmark {
        id: BookmarkId::generate(),
        original_url: url.into(),
        canonical_url: url.into(),
        title: title.into(),
        description: Some("a description line".into()),
        tags: vec!["rust".into(), "cli".into()],
        collection: Some("work".into()),
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

fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn render_loads_state_with_title() {
    use linkmarks_tui::app::App;
    use linkmarks_tui::config::AppConfig;
    use linkmarks_tui::registry::SourceRegistry;
    use std::path::PathBuf;

    let registry = SourceRegistry::resolve(
        linkmarks_tui::config::SourceSelection::All,
        None,
        None,
        Some(PathBuf::from("/nonexistent/store.db")),
    );
    let mut app = App::new(registry, AppConfig::default());
    app.bookmarks = vec![bm("Hello World", "https://example.com/hello")];
    app.selected = 0;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| ui::draw(f, &app))
        .expect("draw");

    let text = buffer_text(&terminal.backend().buffer().clone());
    assert!(text.contains("Hello World"), "expected title in buffer:\n{text}");
    assert!(text.contains("https://example.com/hello"), "expected url in buffer:\n{text}");
}

#[test]
fn render_list_state_shows_status_count() {
    use linkmarks_tui::app::App;
    use linkmarks_tui::config::AppConfig;
    use linkmarks_tui::registry::SourceRegistry;
    use std::path::PathBuf;

    let registry = SourceRegistry::resolve(
        linkmarks_tui::config::SourceSelection::All,
        None,
        None,
        Some(PathBuf::from("/nonexistent/store.db")),
    );
    let mut app = App::new(registry, AppConfig::default());
    app.bookmarks = vec![
        bm("alpha", "https://a"),
        bm("beta", "https://b"),
        bm("gamma", "https://c"),
    ];

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &app)).unwrap();

    let text = buffer_text(&terminal.backend().buffer().clone());
    assert!(text.contains("alpha"));
    assert!(text.contains("beta"));
    assert!(text.contains("gamma"));
    assert!(text.contains("3 / 3 bookmarks"));
}

#[test]
fn render_filter_state_shows_query() {
    use linkmarks_tui::app::App;
    use linkmarks_tui::config::AppConfig;
    use linkmarks_tui::registry::SourceRegistry;
    use linkmarks_tui::state::{AppState, FilterMode};
    use std::path::PathBuf;

    let registry = SourceRegistry::resolve(
        linkmarks_tui::config::SourceSelection::All,
        None,
        None,
        Some(PathBuf::from("/nonexistent/store.db")),
    );
    let mut app = App::new(registry, AppConfig::default());
    app.bookmarks = vec![bm("Rust", "https://rust-lang.org"), bm("Other", "https://x")];
    app.state = AppState::Filter {
        query: "rust".into(),
        mode: FilterMode::Substring,
    };

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &app)).unwrap();

    let text = buffer_text(&terminal.backend().buffer().clone());
    assert!(text.contains("rust"), "expected filter query in buffer:\n{text}");
}

#[test]
fn render_help_overlay() {
    use linkmarks_tui::app::App;
    use linkmarks_tui::config::AppConfig;
    use linkmarks_tui::registry::SourceRegistry;
    use linkmarks_tui::state::AppState;
    use std::path::PathBuf;

    let registry = SourceRegistry::resolve(
        linkmarks_tui::config::SourceSelection::All,
        None,
        None,
        Some(PathBuf::from("/nonexistent/store.db")),
    );
    let mut app = App::new(registry, AppConfig::default());
    app.bookmarks = vec![bm("a", "https://a")];
    app.state = AppState::Help;

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &app)).unwrap();

    let text = buffer_text(&terminal.backend().buffer().clone());
    assert!(text.contains("Help"));
    assert!(text.contains("next row"));
    assert!(text.contains("quit"));
}

#[test]
fn render_empty_selection_shows_placeholder() {
    use linkmarks_tui::app::App;
    use linkmarks_tui::config::AppConfig;
    use linkmarks_tui::registry::SourceRegistry;
    use std::path::PathBuf;

    let registry = SourceRegistry::resolve(
        linkmarks_tui::config::SourceSelection::All,
        None,
        None,
        Some(PathBuf::from("/nonexistent/store.db")),
    );
    let mut app = App::new(registry, AppConfig::default());
    app.bookmarks = vec![];

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &app)).unwrap();

    let text = buffer_text(&terminal.backend().buffer().clone());
    assert!(text.contains("no selection"));
}
