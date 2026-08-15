//! # linkmarks-tui
//!
//! Interactive terminal browser for the LinkMarks store.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod app;
pub mod config;
pub mod filter;
pub mod input;
pub mod registry;
pub mod run;
pub mod sort;
pub mod state;
pub mod ui;

pub use app::App;
pub use config::{AppConfig, SourceSelection};
pub use filter::fuzzy_match;
pub use input::{AppAction, KeyMap};
pub use registry::SourceRegistry;
pub use run::run;
pub use sort::{sort_bookmarks, SortMode};
pub use state::{AppState, FilterMode, PendingAction};
