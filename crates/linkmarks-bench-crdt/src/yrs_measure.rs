//! Encode-size + RSS measurement for yrs v0.20.
//!
//! Pattern (locked decisions #1 + #2):
//! - Per-collection sub-doc: one `yrs::Doc` per collection.
//! - Inside each YDoc: `bookmarks` Map<bookmark_id, Map<field, value>>
//!   and `tags_by_bookmark` Map<bookmark_id, Map<tag, 1>>.
//!
//! We populate from the deterministic fixture, encode the full state
//! per collection (summed across all docs), and report peak RSS via
//! `/proc/self/statm`. Release build + LTO so numbers reflect what
//! users will actually run.

use std::collections::BTreeMap;
use yrs::types::map::MapPrelim;
use yrs::{Doc, Map, ReadTxn, StateVector, Transact, WriteTxn};

use crate::fixture::BenchBookmark;

/// Outcome of one yrs encode run. The byte counts are summed across
/// collection YDocs (a multi-YDoc deployment pattern). RSS is the
/// peak `/proc/self/statm` reading measured after all docs are
/// constructed.
#[derive(Debug, Clone)]
pub struct YrsReport {
    pub total_encoded_bytes: usize,
    pub collection_count: usize,
    pub bookmark_count: usize,
    pub peak_rss_bytes: u64,
    pub per_collection_bytes: Vec<(String, usize)>,
}

/// Build N YDocs (one per collection) from `bookmarks`, encode each,
/// and return the aggregate + RSS reading.
pub fn measure(bookmarks: &[BenchBookmark]) -> YrsReport {
    // Group by collection name. Uncollected bookmarks → "inbox".
    let mut by_collection: BTreeMap<String, Vec<&BenchBookmark>> = BTreeMap::new();
    for b in bookmarks {
        let col = b
            .collection
            .clone()
            .unwrap_or_else(|| "inbox".to_string());
        by_collection.entry(col).or_default().push(b);
    }

    let mut docs: Vec<(String, Doc)> = Vec::with_capacity(by_collection.len());
    let mut per_collection_bytes: Vec<(String, usize)> = Vec::with_capacity(by_collection.len());

    for (col, bms) in &by_collection {
        let doc = Doc::new();
        {
            let mut txn = doc.transact_mut();
            let bookmarks_map = txn.get_or_insert_map("bookmarks");
            let tags_map = txn.get_or_insert_map("tags_by_bookmark");

            for b in bms {
                // Insert bookmark as a nested map.
                let bm = bookmarks_map.insert(&mut txn, b.id.as_str(), MapPrelim::default());
                bm.insert(&mut txn, "original_url", b.original_url.clone());
                bm.insert(&mut txn, "canonical_url", b.canonical_url.clone());
                if let Some(t) = &b.title {
                    bm.insert(&mut txn, "title", t.clone());
                }
                if let Some(d) = &b.description {
                    bm.insert(&mut txn, "description", d.clone());
                }
                bm.insert(&mut txn, "created_at", b.created_at.timestamp_millis());
                bm.insert(&mut txn, "updated_at", b.updated_at.timestamp_millis());
                bm.insert(&mut txn, "source", format!("{:?}", b.source));
                if let Some(ct) = &b.content_type {
                    bm.insert(&mut txn, "content_type", ct.clone());
                }
                bm.insert(&mut txn, "archived", b.archived);

                // Tags as YMap<tag, 1> per bookmark.
                if !b.tags.is_empty() {
                    let tags = tags_map.insert(&mut txn, b.id.as_str(), MapPrelim::default());
                    for tag in &b.tags {
                        tags.insert(&mut txn, tag.as_str(), 1i64);
                    }
                }
            }
            txn.commit();
        }

        // Encode full state for this YDoc. Encoding against an empty
        // StateVector captures the entire doc; encoding against the
        // doc's own state vector would produce nothing (all ops are
        // already known to itself). The encoded bytes are computed in
        // an inner scope so `txn` drops before `doc` is moved into the
        // `docs` vec (yrs::Doc is `!Send` and `Transaction<'_>` borrows
        // it immutably).
        let encoded_len: usize = {
            let txn = doc.transact();
            let sv = StateVector::default();
            txn.encode_state_as_update_v1(&sv).len()
        };
        per_collection_bytes.push((col.clone(), encoded_len));
        docs.push((col.clone(), doc));
    }

    let total_encoded_bytes: usize = per_collection_bytes.iter().map(|(_, n)| n).sum();
    let peak_rss_bytes = read_rss_bytes();

    // Hold docs in scope until after RSS measurement so we measure
    // peak RSS of a fully-populated state.
    std::hint::black_box(&docs);

    YrsReport {
        total_encoded_bytes,
        collection_count: by_collection.len(),
        bookmark_count: bookmarks.len(),
        peak_rss_bytes,
        per_collection_bytes,
    }
}

/// Peak RSS in bytes from `/proc/self/statm`. Field 1 is RSS in pages.
/// Returns 0 on non-Linux (best-effort; spike target is Linux).
fn read_rss_bytes() -> u64 {
    let s = match std::fs::read_to_string("/proc/self/statm") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() < 2 {
        return 0;
    }
    let pages: u64 = match parts[1].parse() {
        Ok(n) => n,
        Err(_) => return 0,
    };
    pages * 4096
}