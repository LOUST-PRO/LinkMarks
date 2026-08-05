//! Firefox bookmark import/export bridge.
//!
//! Reads live Firefox profiles through a read-only `places.sqlite`
//! connection and browser-closed `bookmarks-backups/*.jsonlz4` snapshots.
//! Writes Firefox-shaped, uncompressed JSON snapshots atomically.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod errors;
pub mod jsonlz4;
pub mod places;
pub mod sink;
pub mod source;
pub mod tree;

pub use errors::BridgeError;
pub use jsonlz4::{parse_bytes as parse_jsonlz4_bytes, parse_file as parse_jsonlz4_file};
pub use linkmarks_core::traits::{FailedRecord, Page, WriteReport};
pub use places::parse_places;
pub use sink::FirefoxJsonSink;
pub use source::{discover_default_paths, FirefoxSource};
pub use tree::{FirefoxNode, FirefoxTree};
