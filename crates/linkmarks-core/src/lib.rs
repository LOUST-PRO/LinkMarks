//! # linkmarks-core
//!
//! Domain model, traits, URL canonicalization, and local dedupe for
//! LinkMarks. This crate has no I/O beyond pure parsing helpers —
//! all filesystem, network, and database concerns live in the
//! bridges and the CLI.
//!
//! See:
//! - `model` — `Bookmark`, `SourceRef`, `Collection`, `Tag`
//! - `traits` — `BookmarkSource`, `BookmarkSink`
//! - `canonical` — URL canonicalization rules (ADR-0001)
//! - `dedupe` — local deterministic dedupe by canonical URL
//! - `errors` — error types
//! - `parser` — shared parsing helpers

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod canonical;
pub mod canonical_config;
pub mod dedupe;
pub mod errors;
pub mod model;
pub mod parser;
pub mod storage;
pub mod traits;

pub use canonical::{canonicalize, CanonicalizeError};
pub use dedupe::{dedupe, ConflictRecord, DedupeReport};
pub use errors::CoreError;
pub use model::{Bookmark, BookmarkId, Collection, CollectionId, SourceKind, SourceRef, Tag};
pub use traits::{BookmarkSink, BookmarkSource, Page, WriteReport};

/// Library version (re-exported from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
