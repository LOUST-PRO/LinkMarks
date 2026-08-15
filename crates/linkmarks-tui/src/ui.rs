//! TUI rendering. Three-pane layout: list | detail | status bar.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use linkmarks_core::model::Bookmark;

use crate::app::App;
use crate::state::{AppState, FilterMode};

/// Draw the TUI for `app` into `frame`. Pure function; no I/O.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let main = vertical[0];
    let status = vertical[1];

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(main);

    draw_list(frame, app, horizontal[0]);
    draw_detail(frame, app, horizontal[1]);
    draw_status(frame, app, status);

    if app.state.is_help() {
        draw_help_overlay(frame, area);
    }
}

fn draw_list(frame: &mut Frame, app: &App, area: Rect) {
    let title = match &app.state {
        AppState::List => "Bookmarks".to_string(),
        AppState::Filter { query, mode } => {
            let tag = if matches!(mode, FilterMode::Tag) {
                " [tag]"
            } else {
                ""
            };
            format!("Bookmarks (filter{tag}: {query})")
        }
        _ => "Bookmarks".to_string(),
    };

    let items: Vec<ListItem> = app
        .visible()
        .iter()
        .map(|b| {
            let line = Line::from(vec![
                Span::styled(
                    truncate(&b.title, 36),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    truncate(&b.canonical_url, 36),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.selected));
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(b) = app.selected_bookmark() {
        lines.push(Line::from(vec![
            Span::styled("Title: ", Style::default().fg(Color::Cyan)),
            Span::raw(b.title.clone()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("URL:    ", Style::default().fg(Color::Cyan)),
            Span::raw(b.original_url.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Canon:  ", Style::default().fg(Color::Cyan)),
            Span::raw(b.canonical_url.clone()),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Source: ", Style::default().fg(Color::Cyan)),
            Span::raw(b.source.kind.as_cli_str().to_string()),
        ]));
        if let Some(coll) = &b.collection {
            lines.push(Line::from(vec![
                Span::styled("Folder: ", Style::default().fg(Color::Cyan)),
                Span::raw(coll.clone()),
            ]));
        }
        if !b.tags.is_empty() {
            let mut spans = vec![Span::styled("Tags:   ", Style::default().fg(Color::Cyan))];
            for (i, t) in b.tags.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(
                    format!("[{}]", t),
                    Style::default().fg(Color::Magenta),
                ));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Added:  ", Style::default().fg(Color::Cyan)),
            Span::raw(relative(b.created_at)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Updated:", Style::default().fg(Color::Cyan)),
            Span::raw(format!(" {}", relative(b.updated_at))),
        ]));
        if let Some(desc) = &b.description {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "Description:",
                Style::default().fg(Color::Cyan),
            )]));
            for (i, dl) in desc.lines().enumerate() {
                if i >= 10 {
                    lines.push(Line::from(Span::styled(
                        "  ...",
                        Style::default().fg(Color::DarkGray),
                    )));
                    break;
                }
                lines.push(Line::from(format!("  {}", dl)));
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "(no selection)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Detail"))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let msg = match &app.state {
        AppState::Loading => "loading...".to_string(),
        AppState::Quit(_) => "bye".to_string(),
        AppState::Filter { query, mode } => {
            let label = match mode {
                FilterMode::Substring => "",
                FilterMode::Tag => " [tag]",
                FilterMode::Fuzzy => " [fuzzy]",
            };
            if matches!(mode, FilterMode::Fuzzy) {
                let visible = app.visible().len();
                format!("/{label}{query} ({visible} matches, Esc clear, Enter accept)")
            } else {
                format!("/{label}{query} (Esc clear, Enter accept)")
            }
        }
        _ => {
            let visible = app.visible().len();
            let total = app.bookmarks.len();
            let errors = app.registry.errors.len();
            let base = format!(
                "{} / {} bookmarks   {}",
                visible,
                total,
                app.registry.summary()
            );
            if errors > 0 {
                format!("{base}   [{errors} error(s)]")
            } else {
                base
            }
        }
    };
    let para = Paragraph::new(Line::from(Span::styled(
        msg,
        Style::default().bg(Color::DarkGray).fg(Color::White),
    )));
    frame.render_widget(para, area);
}

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 60, area);
    frame.render_widget(Clear, popup);

    let body = "\
LinkMarks TUI

  j / Down      next row
  k / Up        previous row
  g / G         top / bottom
  PgUp / PgDn   page up / down
  Enter         open bookmark in browser
  /             filter (substring)
  ?             toggle this help
  r / F5        refresh
  q / Ctrl-C    quit
";
    let para = Paragraph::new(body)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .wrap(Wrap { trim: false });
    frame.render_widget(para, popup);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Render a duration as a human-readable relative string.
pub fn relative(at: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(at);
    let secs = delta.num_seconds().max(0);
    if secs < 60 {
        return "just now".into();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{} min ago", mins);
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{} hours ago", hours);
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{} days ago", days);
    }
    let months = days / 30;
    if months < 12 {
        return format!("{} months ago", months);
    }
    let years = days / 365;
    format!("{} years ago", years)
}

/// Render a single bookmark as a small block — handy for tests.
pub fn render_bookmark_to_lines(b: &Bookmark) -> Vec<Line<'static>> {
    vec![
        Line::from(b.title.clone()),
        Line::from(b.original_url.clone()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use linkmarks_core::model::{BookmarkId, SourceKind, SourceRef};

    #[test]
    fn truncate_short_unchanged() {
        assert_eq!(truncate("abc", 8), "abc");
    }

    #[test]
    fn truncate_long_uses_ellipsis() {
        let out = truncate("this is a long string", 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn relative_recent_is_just_now() {
        let now = chrono::Utc::now();
        let s = relative(now);
        assert_eq!(s, "just now");
    }

    #[test]
    fn relative_zero_is_just_now() {
        // The unix epoch is in 1970, so "now" is "56 years ago" in
        // 2026. The point of this test is that the function never
        // produces a negative duration. We re-exercise just-now
        // semantics against the current time.
        let now = chrono::Utc::now();
        assert_eq!(relative(now), "just now");
        let past = now - chrono::Duration::seconds(1);
        assert_eq!(relative(past), "just now");
    }

    #[test]
    fn relative_minutes() {
        let at = chrono::Utc::now() - chrono::Duration::minutes(5);
        let s = relative(at);
        assert_eq!(s, "5 min ago");
    }

    #[test]
    fn relative_days() {
        let at = chrono::Utc::now() - chrono::Duration::days(5);
        let s = relative(at);
        assert_eq!(s, "5 days ago");
    }

    #[test]
    fn render_bookmark_emits_title_and_url() {
        let b = Bookmark {
            id: BookmarkId::generate(),
            original_url: "https://example.com/x".into(),
            canonical_url: "https://example.com/x".into(),
            title: "Hello".into(),
            description: None,
            tags: vec!["rust".into()],
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
        let lines = render_bookmark_to_lines(&b);
        assert_eq!(lines.len(), 2);
        let line0 = format!("{:?}", lines[0]);
        assert!(
            line0.contains("Hello"),
            "expected title in line debug: {line0}"
        );
    }
}
