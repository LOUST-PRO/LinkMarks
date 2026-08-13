//! # linkmarks-bridge-netscape
//!
//! Implements [`BookmarkSource`] and [`BookmarkSink`] for the
//! standard Netscape bookmark HTML format
//! (`<!DOCTYPE NETSCAPE-Bookmark-file-1>`).
//!
//! This format is the de-facto interchange format for browser
//! bookmarks. It is **exported** by Chrome, Firefox, Edge, Brave,
//! Arc, Vivaldi, and Opera; and **imported** by Pinboard,
//! Linkwarden, Shaarli, Raindrop, and most bookmark managers.
//!
//! ## Choice of HTML parser
//!
//! We use [`quick-xml`](https://crates.io/crates/quick-xml) (with
//! the `serialize` feature kept on for symmetry with future
//! structured output needs). The Netscape format is well-formed
//! and predictable: `<DL>` lists with `<DT>` headers, `<DT><A>`
//! links, optional `<DD>` descriptions, optional `<H3>` folder
//! names, and a small set of attributes (`HREF`, `ADD_DATE`,
//! `LAST_MODIFIED`, `TAGS`, `ICON_URI`). We do **not** need a
//! full HTML5 spec-compliant parser (`html5ever` /
//! `markup5ever`) — Netscape files do not include `<script>`,
//! malformed entities, or CSS. Keeping `quick-xml` preserves the
//! "small dep" posture of the rest of the workspace.
//!
//! ## Folder flattening (v1)
//!
//! Netscape `<DT><H3>` folder headings are flattened into
//! synthetic `#folder/<name>` tags on each contained bookmark.
//! This avoids adding a first-class `Collection` field on the
//! model (collections are stubbed as of Fase 2 F1) while still
//! preserving the user's organization as tags. A follow-up
//! after the Collection model is introduced will preserve nested
//! folders as proper `Collection` records. See the `parser` and
//! `sink` module docs for the write-back convention.
//!
//! See [`NetscapeSource`] for the read side and [`NetscapeSink`]
//! for the write side.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod errors;
pub mod parser;
pub mod sink;
pub mod source;

pub use errors::BridgeError;
pub use parser::{
    discover_default_paths, parse, parse_and_flatten, parse_file, NetscapeBookmarks, ParseError,
};
pub use sink::NetscapeSink;
pub use source::NetscapeSource;

pub use linkmarks_core::traits::{FailedRecord, Page, WriteReport};
