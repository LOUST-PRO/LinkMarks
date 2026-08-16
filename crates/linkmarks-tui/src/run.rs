//! TUI loop — owns the terminal, draws frames, reads keys, and
//! restores everything on exit.

use std::io::Stdout;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{self, KeyEvent};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tracing::{info, warn};

use crate::app::App;
use crate::config::AppConfig;
use crate::input::AppAction;
use crate::registry::SourceRegistry;
use crate::ui;

/// Run the TUI. Returns the exit code (0 on clean Quit, 1 on error).
pub fn run(registry: SourceRegistry, config: AppConfig) -> Result<i32> {
    let app = App::new(registry, config);

    let mut terminal = enable_terminal().context("enable raw mode + alternate screen")?;

    let result = loop_main(&mut terminal, app);

    // Cleanup on every exit path.
    cleanup_terminal(&mut terminal);

    match result {
        Ok(code) => Ok(code),
        Err(e) => {
            warn!(error = %e, "TUI loop error");
            Ok(1)
        }
    }
}

fn enable_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enable raw mode")?;
    std::io::stdout()
        .execute(EnterAlternateScreen)
        .context("enter alternate screen")?;
    let backend = CrosstermBackend::new(std::io::stdout());
    Terminal::new(backend).context("create terminal")
}

/// Explicit cleanup. Idempotent. Order matters: leave alternate
/// screen first, then disable raw mode.
fn cleanup_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) {
    let _ = terminal.show_cursor();
    let _ = std::io::stdout().execute(LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

fn loop_main(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> Result<i32> {
    let max_fps = u64::from(app.config.max_fps.clamp(1, 120));
    let frame_budget = Duration::from_millis((1000 / max_fps).max(1));

    let mut last_draw = Instant::now();
    loop {
        let elapsed = last_draw.elapsed();
        let poll = if elapsed >= frame_budget {
            Duration::ZERO
        } else {
            frame_budget - elapsed
        };
        let poll = poll.max(Duration::from_millis(10));

        match event::poll(poll) {
            Ok(true) => {
                if let Ok(event::Event::Key(k)) = event::read() {
                    handle_key(&mut app, k);
                }
            }
            Ok(false) => {
                let _ = app.on_tick();
            }
            Err(e) => {
                warn!(error = %e, "event::poll failed; continuing");
            }
        }

        terminal
            .draw(|f| ui::draw(f, &app))
            .context("ratatui draw")?;
        last_draw = Instant::now();

        if let Some(code) = app.state.exit_code() {
            return Ok(code);
        }
    }
}

fn handle_key(app: &mut App, key: KeyEvent) {
    let action = app.on_key(key);
    match action {
        AppAction::Continue
        | AppAction::Quit(_)
        | AppAction::Refresh
        | AppAction::ShowFilter
        | AppAction::ShowHelp
        | AppAction::ExitFilter
        | AppAction::FilterChar(_)
        | AppAction::FilterBackspace
        | AppAction::FilterAccept
        | AppAction::CycleFilterMode
        | AppAction::CycleSort
        | AppAction::MoveUp
        | AppAction::MoveDown
        | AppAction::PageUp
        | AppAction::PageDown
        | AppAction::Top
        | AppAction::Bottom => {
            // Handled inside App::on_key.
        }
        AppAction::OpenUrl(url) => {
            info!(url, "opening URL in system browser");
        }
    }
}

#[cfg(test)]
mod tests {
    // No TTY-touching tests here — the run loop is exercised by the
    // manual smoke check; the unit tests for the App FSM live in
    // `app.rs` and the integration tests live in `tests/`.
    #[test]
    fn module_compiles() {
        // Existence check: the public API compiles and is reachable.
    }
}
