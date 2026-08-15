//! Bridge-local error wrappers.
//!
//! The bridge converts its own errors into [`linkmarks_core::errors::CoreError`]
//! at the trait boundary. The bridges keep their own typed
//! errors internally so callers (tests, fixtures) can match on
//! concrete variants without losing information.

use linkmarks_core::errors::CoreError;
use thiserror::Error;

/// Errors produced by parsing Netscape HTML.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// I/O failure reading or writing a file.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// XML parse failure (quick-xml).
    #[error("xml: {0}")]
    Xml(String),

    /// A bookmark record could not be turned into a
    /// `linkmarks_core::model::Bookmark` (e.g., canonicalize
    /// failure). Non-fatal — see `parse_and_flatten`.
    #[error("partial failure at {element}: {reason}")]
    Partial {
        /// URL or external-id of the offending element.
        element: String,
        /// Human-readable reason.
        reason: String,
    },

    /// A precondition that should always hold at runtime failed.
    /// Bridges never crash the host process on these; they surface
    /// them as `BridgeError::Invariant`.
    #[error("invariant: {0}")]
    Invariant(String),
}

impl From<BridgeError> for CoreError {
    fn from(err: BridgeError) -> Self {
        match err {
            BridgeError::Io(io) => CoreError::Io(io),
            BridgeError::Xml(s) => CoreError::Storage(format!("netscape xml: {s}")),
            BridgeError::Partial { element, reason } => CoreError::Partial { element, reason },
            BridgeError::Invariant(s) => CoreError::Storage(format!("netscape invariant: {s}")),
        }
    }
}
