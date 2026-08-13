//! Typed errors for the Firefox bridge.

use linkmarks_core::errors::CoreError;
use thiserror::Error;

/// Errors emitted while reading or writing Firefox bookmark data.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// Opening `places.sqlite` read-only failed.
    #[error("sqlite open: {0}")]
    SqliteOpen(#[source] rusqlite::Error),
    /// Querying Firefox's Places schema failed.
    #[error("sqlite query: {0}")]
    SqliteQuery(#[source] rusqlite::Error),
    /// The jsonlz4 magic or size prefix is invalid.
    #[error("jsonlz4 header: {0}")]
    Jsonlz4Header(String),
    /// Raw LZ4 block decompression failed.
    #[error("jsonlz4 decompress: {0}")]
    Jsonlz4Decompress(String),
    /// The decompressed Firefox JSON is invalid.
    #[error("json parse: {0}")]
    JsonParse(#[from] serde_json::Error),
    /// Filesystem I/O failed.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

impl From<BridgeError> for CoreError {
    fn from(error: BridgeError) -> Self {
        match error {
            BridgeError::JsonParse(error) => CoreError::Json(error),
            BridgeError::Io(error) => CoreError::Io(error),
            other => CoreError::Storage(format!("firefox bridge: {other}")),
        }
    }
}
