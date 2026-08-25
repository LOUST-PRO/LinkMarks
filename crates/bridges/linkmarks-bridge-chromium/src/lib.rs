//! # linkmarks-bridge-chromium
//!
//! Implements `BookmarkSource` for the Chromium-family `Bookmarks`
//! JSON file (Google Chrome, Brave, Edge, Arc, Vivaldi, Opera).
//!
//! The file format is identical across these browsers — they all
//! inherit from Chromium's bookmarks format. The parser is tolerant
//! to missing fields and reports per-element failures without
//! aborting the whole import.
//!
//! See `parser.rs` for the JSON shape, `source.rs` for the
//! `ChromiumSource` type that implements `BookmarkSource`, and
//! `sink.rs` for the write-back exporter `ChromiumSink`.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod parser;
pub mod sink;
pub mod source;

pub use parser::{
    chromium_timestamp, parse_and_flatten, BookmarkNode, ChromiumBookmarks, ParseError, Roots,
};
pub use sink::{ChromiumSink, ChromiumTreeFlatten};
pub use source::{discover_default_paths, ChromiumSource};
