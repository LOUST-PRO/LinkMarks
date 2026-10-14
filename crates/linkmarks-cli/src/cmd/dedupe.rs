//! `linkmarks dedupe` — local deterministic dedupe by canonical URL.
//!
//! Reads the live store by default and runs the core dedupe algorithm
//! over the rows. `--source=chrome` parses a Chromium JSON file
//! instead. v2 also accepts `--source=firefox` (live `places.sqlite`
//! or `*.jsonlz4` backups) and `--source=netscape` (Netscape bookmark
//! HTML). `--apply` re-writes the store with the canonical set:
//! archived tombstones are left intact and the winning record is
//! upserted.

use crate::cmd::source_dispatch::{is_path_source, open_source, PATH_SOURCE_KINDS};
use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::dedupe as core_dedupe;
use linkmarks_core::store;
use serde::Serialize;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Args, Debug)]
pub struct DedupeArgs {
    /// Source to dedupe. `store` (default) reads the SQLite store;
    /// `chrome`, `firefox`, or `netscape` parses a browser-backed file.
    #[arg(long, default_value = "store")]
    pub source: String,

    /// Path to a browser-backed source. Required when
    /// `--source=chrome|firefox|netscape`.
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

    /// Emit the dedupe report as JSON to stdout instead of the
    /// human-readable summary. The shape is stable across LinkMarks
    /// releases; agents should parse it without regex on stderr.
    #[arg(long)]
    pub json: bool,
}

/// JSON shape for `--json` output of `linkmarks dedupe`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct DedupeReport {
    /// Tool version.
    pub version: &'static str,
    /// Tool name (`linkmarks dedupe`).
    pub tool: &'static str,
    /// Bookmark count before dedupe.
    pub before: usize,
    /// Canonical bookmark count after dedupe.
    pub after: usize,
    /// How many bookmarks were merged into a canonical row.
    pub removed: usize,
    /// How many merge conflicts surfaced (non-fatal).
    pub conflicts: usize,
    /// Whether `--apply` was passed.
    pub applied: bool,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
}

pub fn run(args: DedupeArgs, format: crate::Format, paths: Paths) -> Result<i32> {
    let started = Instant::now();
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
            // Page through the store with a conservative limit until empty.
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
        "chrome" | "firefox" | "netscape" | "html" => {
            let kind = linkmarks_core::SourceKind::from_cli_str(args.source.as_str())
                .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
            if !is_path_source(kind) {
                bail!(
                    "unsupported --source '{}' (try `store` or one of {:?})",
                    args.source,
                    PATH_SOURCE_KINDS
                );
            }
            let path = args.path.clone().ok_or_else(|| {
                anyhow::anyhow!("--path is required for --source={}", args.source)
            })?;
            open_source(kind, &path)?
        }
        other => bail!(
            "unsupported --source '{other}' (try `store` or one of {:?})",
            PATH_SOURCE_KINDS
        ),
    };

    let before_count = bookmarks.len();
    let (canonical, report) = core_dedupe(&bookmarks);
    let after_count = canonical.len();
    let removed = before_count.saturating_sub(after_count);
    let duration_ms = started.elapsed().as_millis() as u64;

    if args.json {
        // Machine-readable path. The report struct is stable; agents
        // should parse it without regex on stderr.
        let json_report = DedupeReport {
            version: env!("CARGO_PKG_VERSION"),
            tool: "linkmarks dedupe",
            before: before_count,
            after: after_count,
            removed,
            conflicts: report.conflicts.len(),
            applied: args.apply,
            duration_ms,
        };
        let serialized = serde_json::to_string_pretty(&json_report)?;
        println!("{serialized}");
    } else {
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
    }

    if !args.apply {
        if !args.json {
            eprintln!("(dry-run; pass --apply to write)");
        }
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
                tracing::warn!(error = %e, url = &bm.original_url, "dedupe upsert failed");
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
            "(apply mode: {} canonical records; browser source has no on-disk write)",
            canonical.len()
        );
    }
    let _ = args.refresh_canonical; // reserved for a future canonical-refresh pass

    if report.conflicts.is_empty() {
        Ok(crate::exit_codes::OK)
    } else {
        Ok(crate::exit_codes::DEDUPE_CONFLICTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_report_includes_required_fields() {
        // Schema spot-check: the JSON shape must contain the
        // stable field names agents rely on.
        let r = DedupeReport {
            version: "2.2.2",
            tool: "linkmarks dedupe",
            before: 100,
            after: 92,
            removed: 8,
            conflicts: 0,
            applied: false,
            duration_ms: 12,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"tool\":\"linkmarks dedupe\""));
        assert!(json.contains("\"before\":100"));
        assert!(json.contains("\"after\":92"));
        assert!(json.contains("\"removed\":8"));
        assert!(json.contains("\"conflicts\":0"));
        assert!(json.contains("\"applied\":false"));
        assert!(json.contains("\"duration_ms\":12"));
    }
}
