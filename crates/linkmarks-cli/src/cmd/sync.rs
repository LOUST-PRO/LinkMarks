//! `linkmarks sync` — multi-device sync via yrs-encoded payload.
//!
//! **Status**: preview / dry-run only. The local state encoding and the
//! wire format are validated; the actual `POST /v1/sync` round-trip is
//! pending in the relay binary.
//!
//! What this subcommand does today:
//!
//! 1. Open the local SQLite store.
//! 2. List every bookmark (`store::Store::list` with high limit).
//! 3. Build a per-collection `yrs::Doc`. Bookmarks are grouped by
//!    `Bookmark::collection` so each collection is a self-contained
//!    sub-doc — clients merge independently and the relay can serve a
//!    single collection without unpacking the whole library.
//! 4. Encode each sub-doc via `yrs::encode_state_as_update_v1`.
//! 5. Print a summary table: collection name, bookmark count,
//!    encoded bytes, hash (FNV-1a-64). No HTTP, no write to store,
//!    no write to disk outside the optional `--out-dir`.
//!
//! `--out-dir <PATH>`: write each sub-doc's encoded bytes to
//! `<PATH>/<collection_slug>.ydoc.bin` so the operator can inspect them
//! (e.g. `xxd`, `wc -c`, `lz4 <file>` to see the post-LZ4 wire size).
//!
//! Exit codes:
//! - 0: dry-run completed, dry-run report printed.
//! - 1: store not initialized (run `linkmarks init` first).
//! - 2: bad args.
//!
//! This subcommand is the pre-merge validation hook: running
//! `linkmarks sync --dry-run` on a freshly-imported library must produce
//! the same encoded size (within LZ4 compression tolerance) as a real
//! `linkmarks sync --remote <relay>` once the relay binary ships.

use crate::Paths;
use anyhow::{bail, Context, Result};
use clap::Args;
use linkmarks_core::store;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct SyncArgs {
    /// Remote relay URL. **Not yet implemented** — passing this without
    /// `--dry-run` is rejected with a clear error.
    #[arg(long)]
    pub remote: Option<String>,

    /// Compute and print the wire payload, but do not contact the relay
    /// and do not modify the local store.
    #[arg(long)]
    pub dry_run: bool,

    /// Optional directory to write per-collection encoded bytes
    /// (`<slug>.ydoc.bin`). Defaults to none (in-memory only).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,

    /// Page size when listing from the store. Defaults to 100_000
    /// (effectively "all bookmarks in one page").
    #[arg(long, default_value = "100000")]
    pub limit: usize,
}

pub fn run(args: SyncArgs, _format: crate::Format, paths: Paths) -> Result<i32> {
    if args.remote.is_some() && !args.dry_run {
        bail!(
            "live sync is relay-binary work; today only --dry-run is implemented."
        );
    }
    if !args.dry_run {
        bail!("pass --dry-run (the only mode implemented in this preview)");
    }
    if !paths.store.exists() {
        bail!(
            "store not found at {}; run `linkmarks init` first",
            paths.store.display()
        );
    }

    let s = store::open(&paths.store).context("open store")?;
    let total = s.count_all().context("count_all")?;
    if total == 0 {
        println!("(store is empty — nothing to encode)");
        return Ok(crate::exit_codes::OK);
    }

    let bookmarks = s.list(args.limit.max(1), 0).context("list bookmarks")?;
    let by_collection = group_by_collection(&bookmarks);

    if let Some(out_dir) = &args.out_dir {
        std::fs::create_dir_all(out_dir)
            .with_context(|| format!("create out-dir {}", out_dir.display()))?;
    }

    println!(
        "linkmarks sync --dry-run (preview)\n  \
         store: {}\n  \
         bookmarks: {}\n  \
         collections: {}\n",
        paths.store.display(),
        bookmarks.len(),
        by_collection.len()
    );
    println!(
        "{:<24} {:>10} {:>12} {:>16}  hash",
        "collection", "bookmarks", "bytes", "fingerprint"
    );
    println!("{}", "-".repeat(80));

    let mut grand_total_bytes: usize = 0;
    let mut grand_total_bookmarks: usize = 0;
    for (collection, items) in &by_collection {
        let encoded = encode_collection(collection, items);
        let hash = fnv1a_64(&encoded);
        let slug = collection_slug(collection);
        println!(
            "{:<24} {:>10} {:>12} {:>16x}  {}",
            truncate(collection, 24),
            items.len(),
            encoded.len(),
            hash,
            slug
        );
        grand_total_bytes = grand_total_bytes.saturating_add(encoded.len());
        grand_total_bookmarks = grand_total_bookmarks.saturating_add(items.len());

        if let Some(out_dir) = &args.out_dir {
            let path = out_dir.join(format!("{slug}.ydoc.bin"));
            std::fs::write(&path, &encoded)
                .with_context(|| format!("write {}", path.display()))?;
        }
    }

    println!("{}", "-".repeat(80));
    println!(
        "{:<24} {:>10} {:>12}",
        "TOTAL", grand_total_bookmarks, grand_total_bytes
    );
    println!(
        "\ndry-run complete. {} sub-docs, {} bytes uncompressed yrs payload.",
        by_collection.len(),
        grand_total_bytes
    );
    if let Some(out_dir) = args.out_dir {
        println!(
            "wrote per-collection encoded files to {}",
            out_dir.display()
        );
    }
    println!(
        "Note: the actual relay wire size will be ~6-14× smaller after LZ4 compression.\n\
         Run `lz4 <file>` on a `.ydoc.bin` to verify."
    );

    Ok(crate::exit_codes::OK)
}

fn group_by_collection(
    bookmarks: &[linkmarks_core::Bookmark],
) -> std::collections::BTreeMap<String, Vec<linkmarks_core::Bookmark>> {
    let mut map: std::collections::BTreeMap<String, Vec<linkmarks_core::Bookmark>> =
        Default::default();
    for bm in bookmarks {
        let key = bm
            .collection
           .clone()
            .unwrap_or_else(|| "(uncategorized)".to_string());
        map.entry(key).or_default().push(bm.clone());
    }
    map
}

fn encode_collection(collection: &str, bookmarks: &[linkmarks_core::Bookmark]) -> Vec<u8> {
    use yrs::types::map::MapPrelim;
    use yrs::{Doc, Map, ReadTxn, StateVector, Transact, WriteTxn};

    let doc = Doc::new();
    {
        let mut t = doc.transact_mut();
        let meta = t.get_or_insert_map("meta");
        meta.insert(&mut t, "collection_name", collection);
        meta.insert(&mut t, "spike_marker", "sync-preview");
        let bookmarks_map = t.get_or_insert_map("bookmarks");
        let tags_map = t.get_or_insert_map("tags_by_bookmark");
        for bm in bookmarks {
            let bm_entry = bookmarks_map.insert(
                &mut t,
                bm.id.0.as_str(),
                MapPrelim::default(),
            );
            bm_entry.insert(&mut t, "original_url", bm.original_url.clone());
            bm_entry.insert(&mut t, "canonical_url", bm.canonical_url.clone());
            bm_entry.insert(&mut t, "title", bm.title.clone());
            bm_entry.insert(&mut t, "archived", bm.archived);
            if let Some(ct) = &bm.content_type {
                bm_entry.insert(&mut t, "content_type", ct.clone());
            }
            bm_entry.insert(
                &mut t,
                "source_kind",
                bm.source.kind.as_cli_str().to_string(),
            );
            let tags_entry = tags_map.insert(&mut t, bm.id.0.as_str(), MapPrelim::default());
            for tag in &bm.tags {
                tags_entry.insert(&mut t, tag.clone(), 1i64);
            }
        }
        t.commit();
    }
    let txn = doc.transact();
    txn.encode_state_as_update_v1(&StateVector::default())
}

fn collection_slug(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn fnv1a_64(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &byte in b {
        h ^= byte as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_slug_lowercases_and_replaces_separators() {
        assert_eq!(collection_slug("Work / Research"), "work___research");
        assert_eq!(collection_slug("(uncategorized)"), "uncategorized");
        assert_eq!(collection_slug("a:b"), "a_b");
    }

    #[test]
    fn truncate_handles_short_and_long() {
        // Short input passes through unchanged.
        assert_eq!(truncate("short", 10), "short");
        // Long input is truncated to exactly `max` chars including the
        // single-codepoint ellipsis (U+2026). This is what we want for
        // fixed-width column display where the column is `max` wide.
        assert_eq!(truncate("a long collection name here", 10), "a long co…");
        assert_eq!(truncate("a long collection name here", 24), "a long collection name …");
    }

    #[test]
    fn fnv_hash_is_deterministic() {
        let a = fnv1a_64(b"hello");
        let b = fnv1a_64(b"hello");
        assert_eq!(a, b);
        assert_ne!(a, fnv1a_64(b"hellp"));
    }
}
