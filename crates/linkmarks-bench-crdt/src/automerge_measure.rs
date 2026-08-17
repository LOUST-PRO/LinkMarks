//! Encode-size + RSS measurement for automerge v0.5.
//!
//! Pattern (locked decisions #1 + #2):
//! - Per-collection sub-doc: one `automerge::Automerge` per collection.
//! - Inside each doc: `bookmarks` Map<bookmark_id, Map<field, value>>
//!   and `tags_by_bookmark` Map<bookmark_id, Map<tag, 1>>.
//!
//! Same shape as `yrs_measure` so encode-size + RSS numbers are
//! directly comparable. Release build + LTO.

use std::collections::BTreeMap;

use automerge::{
    transaction::Transactable, AutomergeError, Automerge, ObjType, ROOT,
};

use crate::fixture::BenchBookmark;

#[derive(Debug, Clone)]
pub struct AutomergeReport {
    pub total_encoded_bytes: usize,
    pub collection_count: usize,
    pub bookmark_count: usize,
    pub peak_rss_bytes: u64,
    pub per_collection_bytes: Vec<(String, usize)>,
}

pub fn measure(bookmarks: &[BenchBookmark]) -> Result<AutomergeReport, AutomergeError> {
    let mut by_collection: BTreeMap<String, Vec<&BenchBookmark>> = BTreeMap::new();
    for b in bookmarks {
        let col = b
            .collection
            .clone()
            .unwrap_or_else(|| "inbox".to_string());
        by_collection.entry(col).or_default().push(b);
    }

    let mut docs: Vec<(String, Automerge)> = Vec::with_capacity(by_collection.len());
    let mut per_collection_bytes: Vec<(String, usize)> = Vec::with_capacity(by_collection.len());

    for (col, bms) in &by_collection {
        let mut doc = Automerge::new();
        {
            let mut tx = doc.transaction();
            let bookmarks_map = tx.put_object(ROOT, "bookmarks", ObjType::Map)?;
            let tags_map = tx.put_object(ROOT, "tags_by_bookmark", ObjType::Map)?;

            for b in bms {
                let bm = tx.put_object(&bookmarks_map, &b.id, ObjType::Map)?;
                tx.put(&bm, "original_url", b.original_url.clone())?;
                tx.put(&bm, "canonical_url", b.canonical_url.clone())?;
                if let Some(t) = &b.title {
                    tx.put(&bm, "title", t.clone())?;
                }
                if let Some(d) = &b.description {
                    tx.put(&bm, "description", d.clone())?;
                }
                tx.put(&bm, "created_at", b.created_at.timestamp_millis())?;
                tx.put(&bm, "updated_at", b.updated_at.timestamp_millis())?;
                tx.put(&bm, "source", format!("{:?}", b.source))?;
                if let Some(ct) = &b.content_type {
                    tx.put(&bm, "content_type", ct.clone())?;
                }
                tx.put(&bm, "archived", b.archived)?;

                if !b.tags.is_empty() {
                    let tags = tx.put_object(&tags_map, &b.id, ObjType::Map)?;
                    for tag in &b.tags {
                        tx.put(&tags, tag.as_str(), 1i64)?;
                    }
                }
            }
            tx.commit();
        }

        let encoded = doc.save();
        per_collection_bytes.push((col.clone(), encoded.len()));
        docs.push((col.clone(), doc));
    }

    let total_encoded_bytes: usize = per_collection_bytes.iter().map(|(_, n)| n).sum();
    let peak_rss_bytes = read_rss_bytes();
    std::hint::black_box(&docs);

    Ok(AutomergeReport {
        total_encoded_bytes,
        collection_count: by_collection.len(),
        bookmark_count: bookmarks.len(),
        peak_rss_bytes,
        per_collection_bytes,
    })
}

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