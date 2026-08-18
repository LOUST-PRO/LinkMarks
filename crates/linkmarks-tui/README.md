# linkmarks-tui

Interactive terminal browser for the local LinkMarks store. Built on
ratatui + crossterm with fuzzy filter (nucleo) and four sort modes
(canonical URL, title, created DESC, updated DESC).

Launched via `linkmarks tui` from the umbrella binary. This crate ships
the event loop, the `App` state machine, and the `state_wiring_*`
integration tests that enforce that every UI-state enum variant is
reachable from a real keypress.

## What is it

`linkmarks-tui` is the read-only view onto the same SQLite store that
the CLI writes through. It does not own the data; it renders it.

Key bindings (default):

- `/` — enter fuzzy filter
- `s` — cycle sort mode
- `o` — open the highlighted URL in the system browser
- `Enter` — open the highlighted URL
- `q` / `Esc` — quit

The `App` struct is the single source of UI state. Every variant of
state enums (`SortMode`, `FilterMode`, `AppState`) is exercised by a
`state_wiring_*` integration test that emulates the real `KeyEvent`
and verifies both the mutation and the downstream effect.

## Install

```toml
[dependencies]
linkmarks-tui = "2.2.0"
```

To launch interactively from the umbrella:

```bash
cargo install linkmarks --locked
linkmarks tui
```

## Usage

From the umbrella binary:

```bash
linkmarks tui
```

From your own Rust binary:

```rust
use linkmarks_tui::App;

let mut app = App::open(store_path)?;
app.run()?;
```

## Architecture

The crate is structured around a single `App` state machine. The input
handler (`input.rs`) maps `KeyEvent` to `AppAction`, the `App` methods
mutate state, and the renderer (`ui/`) reads state. Tests under
`tests/app_test.rs` and `tests/state_wiring_*.rs` enforce that no
UI-state enum variant is declared but unreachable.

## License

Dual: AGPL-3.0-or-later (open source) + Commercial license for entities
that need to skip the AGPL §13 network-use clause. See `LICENSE` and
`LICENSE-COMMERCIAL.md` at the project root. Contact:
`opensource@loust.pro`.
