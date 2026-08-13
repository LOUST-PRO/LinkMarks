//! # linkmarks-tui
//!
//! Interactive terminal browser for the LinkMarks store.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod app;
pub mod config;
pub mod input;
pub mod registry;
pub mod run;
pub mod state;
pub mod ui;

pub use app::App;
pub use config::{AppConfig, SourceSelection};
pub use input::{AppAction, KeyMap};
pub use registry::SourceRegistry;
pub use run::run;
pub use state::{AppState, FilterMode, PendingAction};
