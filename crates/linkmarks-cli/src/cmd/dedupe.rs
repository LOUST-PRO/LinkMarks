//! `linkmarks dedupe` — local deterministic dedupe by canonical URL.
//!
//! Reads the live store by default (Fase 2) and runs the core dedupe
//! algorithm over the rows. `--source=chrome` retains the Fase-1
//! behaviour (parse a Chromium JSON file). `--apply` re-writes the
//! store with the canonical set: archived tombstones are left intact
//! and the winning record is upserted.

use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::dedupe as core_dedupe;
use linkmarks_core::store;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct DedupeArgs {
    /// Source to dedupe. `store` (default) reads the SQLite store;
    /// `chrome` parses a Chromium JSON file.
    #[arg(long, default_value = "store")]
    pub source: String,

    /// Path to a Chromium JSON source. Required when `--source=chrome`.
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Apply the merge (default is dry-run).
    ///
    /// Without `--apply`, the command only reports. The gate exists
    /// because destructive merges are irreversible without a backup.
    #[arg(long)]
    pub apply: bool,

    /// Refresh the canonical_url on every row by re-running the loader
    /// config. Off by default — useful after editing config.toml to
    /// tighten rules for a specific host.
    #[arg(long)]
    pub refresh_canonical: bool,
}

pub fn run(args: DedupeArgs, format: crate::Format, paths: Paths) -> Result<i32> {
    let bookmarks = match args.source.as_str() {
        "store" => {
            if !paths.store.exists() {
                bail!(
                    "store not found at {}; run `linkmarks init` first",
                    paths.store.display()
                );
            }
            let s = store::open(&paths.store)?;
            let mut all = Vec::new();
            // Phase-2 store has small row counts; page through with a
            // conservative limit until empty.
            let mut offset = 0usize;
            loop {
                let page = s.list(500, offset)?;
                if page.is_empty() {
                    break;
                }
                let page_len = page.len();
                offset += page_len;
                all.extend(page);
                if page_len < 500 {
                    break;
                }
            }
            all
        }
        "chrome" => {
            let path = args
                .path
                .clone()
                .ok_or_else(|| anyhow::anyhow!("--path is required for --source=chrome"))?;
            let src = linkmarks_bridge_chromium::ChromiumSource::open(&path)?;
            src.list()?
        }
        other => bail!("unsupported --source '{other}' (try `store` or `chrome`)"),
    };

    let (canonical, report) = core_dedupe(&bookmarks);

    match format {
        crate::Format::Table => {
            println!(
                "canonical_count={} merged={} conflicts={}",
                report.canonical_count,
                report.merged_count,
                report.conflicts.len()
            );
            for c in &report.conflicts {
                println!(
                    "- {}\n    chosen={}\n    conflicting={:?}\n    differing_fields={:?}",
                    c.canonical_url, c.chosen_id, c.conflicting_ids, c.differing_fields
                );
            }
        }
        crate::Format::Json => {
            let mut out = serde_json::to_string_pretty(&report)?;
            out.push('\n');
            print!("{out}");
        }
        crate::Format::Yaml => {
            let value = serde_yaml::to_string(&report)?;
            print!("{value}");
        }
    }

    if !args.apply {
        eprintln!("(dry-run; pass --apply to write)");
        if report.conflicts.is_empty() {
            return Ok(crate::exit_codes::OK);
        }
        return Ok(crate::exit_codes::DEDUPE_CONFLICTS);
    }

    if args.source == "store" {
        let mut s = store::open(&paths.store)?;
        let mut rewritten = 0usize;
        for bm in &canonical {
            if let Err(e) = s.upsert(bm) {
                tracing::warn!(error = %e, url = %bm.original_url, "dedupe upsert failed");
            } else {
                rewritten += 1;
            }
        }
        eprintln!(
            "(apply mode: {} canonical records, {} rows upserted back into the store)",
            canonical.len(),
            rewritten
        );
    } else {
        eprintln!(
            "(apply mode: {} canonical records; Fase-1 chrome source has no on-disk write)",
            canonical.len()
        );
    }
    let _ = args.refresh_canonical; // reserved for Fase 3+

    if report.conflicts.is_empty() {
        Ok(crate::exit_codes::OK)
    } else {
        Ok(crate::exit_codes::DEDUPE_CONFLICTS)
    }
}
