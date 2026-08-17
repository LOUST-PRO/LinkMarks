//! Source/sink traits.
//!
//! Bridges implement these. The CLI orchestrates them via
//! `SourceRegistry` (CLI crate). Plugins implement the same
//! traits via `libloading`.

use crate::errors::CoreError;
use crate::model::{Bookmark, SourceKind};
use serde::{Deserialize, Serialize};

/// A paginated result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    /// Bookmarks in this page, sorted by canonical URL then id.
    pub items: Vec<Bookmark>,
    /// Opaque cursor for the next page; `None` when exhausted.
    pub next_cursor: Option<String>,
}

/// Report from a sink write operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WriteReport {
    /// Number of records successfully written.
    pub written: usize,
    /// Number of records that failed (per-element; non-fatal).
    pub failed: Vec<FailedRecord>,
}

/// One record that a sink could not write.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedRecord {
    /// Identifier (URL, external_id) of the failed record.
    pub id: String,
    /// Reason for failure.
    pub reason: String,
}

/// Read-side trait. Implemented by every bridge.
pub trait BookmarkSource: Send + Sync {
    /// What kind of source this is.
    fn kind(&self) -> SourceKind;

    /// List all bookmarks. May be expensive for large stores; prefer
    /// `list_paginated` for production reads.
    fn list(&self) -> Result<Vec<Bookmark>, CoreError>;

    /// Paginated listing. Cursor is opaque; first call uses `None`.
    fn list_paginated(&self, cursor: Option<String>, limit: usize) -> Result<Page, CoreError>;

    /// Look up a single bookmark by canonical URL.
    fn by_canonical(&self, canonical: &str) -> Result<Option<Bookmark>, CoreError>;
}

/// Write-side trait. Implemented by sinks (Netscape HTML, server).
pub trait BookmarkSink: Send + Sync {
    /// What kind of sink this is.
    fn kind(&self) -> SourceKind;

    /// Write a batch of bookmarks. Returns a report; non-fatal
    /// failures are listed in `WriteReport::failed`.
    fn write(&mut self, bookmarks: &[Bookmark]) -> Result<WriteReport, CoreError>;

    /// Delete a bookmark by external identifier.
    fn delete(&mut self, external_id: &str) -> Result<(), CoreError>;
}
