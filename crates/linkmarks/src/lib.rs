//! `linkmarks` — the umbrella library for the LinkMarks bookmark manager.
//!
//! LinkMarks is a local-first, AGPL-licensed bookmark manager that imports
//! from your browser, deduplicates deterministically by canonical URL, and
//! works offline against a single SQLite store under `~/.local/share/linkmarks/`.
//!
//! This crate is the **stable, re-export umbrella**. It bundles the seven
//! LinkMarks sub-libraries — the core domain model, the CLI dispatcher, the
//! terminal UI, and the three browser bridge parsers — into a single crate
//! so downstream Rust code can `use linkmarks::*` without juggling six
//! separate `Cargo.toml` dependencies.
//!
//! ## Module layout
//!
//! - [`linkmarks_core`] — domain model, storage, canonicalization, dedupe.
//! - [`linkmarks_tui`] — interactive terminal browser (ratatui + crossterm).
//! - [`linkmarks_cli`] — clap-based CLI dispatcher. *Not re-exported as a
//!   library from here yet; invoke via `cargo install` of the matching binary
//!   once 2.3.0 transitions it to a dual lib+bin target.* For 2.2.0 the
//!   binary lives in the `linkmarks-cli` workspace member and is installed
//!   via `cargo install --path crates/linkmarks-cli --bin linkmarks`.
//! - `linkmarks_bridge_chromium` / `linkmarks_bridge_firefox` /
//!   `linkmarks_bridge_netscape` — the three interchange parsers.
//!
//! ## Why an umbrella?
//!
//! Pre-2.2.0, the seven members were each published to crates.io as
//! independent artifacts, which produced six pages of metadata-fragmented
//! entries for what users perceive as a single tool. 2.2.0 collapses this
//! to one umbrella artifact (`linkmarks`); the sub-libraries remain
//! installable via the umbrella's `path = "..."` deps but no longer ship
//! their own crates.io pages.
//!
//! ## License
//!
//! Dual-licensed under AGPL-3.0-or-later (open source) or a commercial
//! license. See `LICENSE` at the repository root.

pub use linkmarks_core;
pub use linkmarks_tui;
