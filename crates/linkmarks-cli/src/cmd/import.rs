//! `linkmarks import` — import bookmarks from a source file into the store.
//!
//! Pipeline:
//! 1. Open the source via [`crate::cmd::source_dispatch::open_source`].
//!    Chromium JSON, Firefox `places.sqlite`, Firefox `jsonlz4` backups,
//!    and Netscape bookmark HTML are all supported in v2.
//! 2. Read bookmarks via `BookmarkSource::list()`.
//! 3. Canonicalize each URL through the loader-supplied
//!    [`CanonicalConfig`](linkmarks_core::CanonicalConfig).
//! 4. Upsert into the local store.
//!
//! `--dry-run` parses and canonicalizes but does not write the store.
//! `--source=store` is rejected — `import` always writes, never reads,
//! from the store. Network-backed sources (Pinboard / Linkwarden) are
//! rejected because this CLI build does not link their SDKs.

use crate::cmd::source_dispatch::{is_path_source, open_source, PATH_SOURCE_KINDS};
use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::canonical::canonicalize_with;
use linkmarks_core::config::load_from;
use linkmarks_core::store;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// Source to import from. Accepted values: `chrome` (Chromium JSON,
    /// including Chrome/Brave/Edge/Arc/Vivaldi/Opera aliases), `firefox`
    /// (live `places.sqlite` or `*.jsonlz4` backup), or `netscape`
    /// (Netscape bookmark HTML).
    #[arg(long, default_value = "chrome")]
    pub source: String,

    /// Path to the source file.
    #[arg(long)]
    pub path: PathBuf,

    /// Parse and canonicalize but do not write the store.
    #[arg(long)]
    pub dry_run: bool,

    /// Overwrite the destination store with a fresh DB. Off by
    /// default; the operator decides when to discard history.
    #[arg(long)]
    pub fresh: bool,
}

pub fn run(args: ImportArgs, _format: crate::Format, paths: Paths) -> Result<i32> {
    let kind = linkmarks_core::SourceKind::from_cli_str(&args.source)
        .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
    if !is_path_source(kind) {
        bail!(
            "--source={} is not importable from this CLI build; try one of {:?}",
            args.source,
            PATH_SOURCE_KINDS
        );
    }
    if !args.path.exists() {
        bail!("source file not found: {}", args.path.display());
    }
    if args.fresh && args.dry_run {
        bail!("--fresh and --dry-run are mutually exclusive");
    }

    let bookmarks = open_source(kind, &args.path)?;
    let cfg = load_from(&paths.config)?;
    let report = canonicalize_bookmarks(&bookmarks, &cfg);

    if args.dry_run {
        println!(
            "imported (dry-run) {} bookmarks from {} ({})\n  parsed_ok={} canonical_ok={}",
            bookmarks.len(),
            args.path.display(),
            args.source,
            report.parsed_ok,
            report.canonical_ok
        );
        return Ok(crate::exit_codes::OK);
    }

    if args.fresh && paths.store.exists() {
        std::fs::remove_file(&paths.store)
            .map_err(|e| anyhow::anyhow!("remove {}: {e}", paths.store.display()))?;
    }
    let mut s = store::open(&paths.store)?;
    let mut written = 0usize;
    let mut failed = 0usize;
    for bm in &report.canonical {
        match s.upsert(bm) {
            Ok(_) => written += 1,
            Err(e) => {
                failed += 1;
                tracing::warn!(error = %e, url = %bm.original_url, "upsert failed");
            }
        }
    }

    println!(
        "imported {} bookmarks from {} ({})\n  parsed_ok={} canonical_ok={} written={} failed={}",
        bookmarks.len(),
        args.path.display(),
        args.source,
        report.parsed_ok,
        report.canonical_ok,
        written,
        failed
    );

    if failed == 0 {
        Ok(crate::exit_codes::OK)
    } else {
        Ok(crate::exit_codes::PARTIAL)
    }
}

/// Aggregated outcome of canonicalization: original bookmarks plus the
/// successfully canonicalized subset and counters for diagnostics.
struct CanonicalReport {
    /// Bookmarks with a freshly-computed `canonical_url`.
    canonical: Vec<linkmarks_core::Bookmark>,
    /// Number of records the source parser emitted.
    parsed_ok: usize,
    /// Number of records that survived canonicalization.
    canonical_ok: usize,
}

fn canonicalize_bookmarks(
    bookmarks: &[linkmarks_core::Bookmark],
    cfg: &linkmarks_core::CanonicalConfig,
) -> CanonicalReport {
    let parsed_ok = bookmarks.len();
    let mut canonical = Vec::with_capacity(bookmarks.len());
    let mut canonical_ok = 0usize;
    for bm in bookmarks {
        match canonicalize_with(&bm.original_url, cfg) {
            Ok(canonical_url) => {
                let mut updated = bm.clone();
                updated.canonical_url = canonical_url;
                canonical.push(updated);
                canonical_ok += 1;
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    url = &bm.original_url,
                    "canonicalize failed; keeping as-is"
                );
                canonical.push(bm.clone());
            }
        }
    }
    CanonicalReport {
        canonical,
        parsed_ok,
        canonical_ok,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use linkmarks_core::canonical_config::CanonicalConfig;
    use linkmarks_core::model::{Bookmark, BookmarkId, SourceKind, SourceRef};

    fn mk_bm(url: &str) -> Bookmark {
        Bookmark {
            id: BookmarkId::generate(),
            original_url: url.into(),
            canonical_url: url.into(),
            title: "t".into(),
            description: None,
            tags: vec![],
            collection: None,
            created_at: Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: Utc.timestamp_opt(0, 0).unwrap(),
            source: SourceRef {
                kind: SourceKind::Manual,
                external_id: None,
                imported_at: Utc.timestamp_opt(0, 0).unwrap(),
                raw: None,
            },
            content_type: None,
            archived: false,
        }
    }

    #[test]
    fn canonicalize_lowercases_host_and_strips_tracking() {
        let cfg = CanonicalConfig::default_rules();
        let bms = vec![mk_bm("HTTPS://Example.com/p?utm_source=x&id=42")];
        let report = canonicalize_bookmarks(&bms, &cfg);
        assert_eq!(report.canonical.len(), 1);
        assert!(report.canonical[0]
            .canonical_url
            .starts_with("https://example.com/"));
        assert!(report.canonical[0].canonical_url.contains("id=42"));
        assert!(!report.canonical[0].canonical_url.contains("utm_source"));
    }

    #[test]
    fn from_cli_str_accepts_firefox_and_netscape_aliases() {
        // Spot-check the dispatch accepts the new sources at the parse layer.
        assert_eq!(
            linkmarks_core::SourceKind::from_cli_str("firefox"),
            Some(SourceKind::Firefox)
        );
        assert_eq!(
            linkmarks_core::SourceKind::from_cli_str("netscape"),
            Some(SourceKind::Netscape)
        );
        assert_eq!(
            linkmarks_core::SourceKind::from_cli_str("html"),
            Some(SourceKind::Netscape)
        );
    }
}