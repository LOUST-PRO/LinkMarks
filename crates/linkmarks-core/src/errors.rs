//! Error types for `linkmarks-core`.

use thiserror::Error;

/// Top-level error type for core operations.
#[derive(Debug, Error)]
pub enum CoreError {
    /// A bookmark URL could not be parsed.
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    /// A canonicalization rule failed (e.g., IDN with non-ASCII host).
    #[error("canonicalize failed: {0}")]
    Canonicalize(String),

    /// I/O error from the underlying reader/writer.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error.
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    /// A bridge or sink reported a non-fatal element failure. The
    /// `element` field carries the affected input identifier (URL or
    /// external_id) for reporting.
    #[error("partial failure at element {element}: {reason}")]
    Partial {
        /// Identifier of the offending element (URL, external_id, etc).
        element: String,
        /// Human-readable reason.
        reason: String,
    },
}
