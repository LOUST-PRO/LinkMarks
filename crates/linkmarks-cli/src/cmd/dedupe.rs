//! `linkmarks dedupe` — local deterministic dedupe by canonical URL.

use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::dedupe as core_dedupe;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct DedupeArgs {
    /// Source to dedupe. v1 supports `chrome`.
    #[arg(long, default_value = "chrome")]
    pub source: String,

    /// Path to the source file.
    #[arg(long)]
    pub path: PathBuf,

    /// Apply the merge (default is dry-run).
    ///
    /// This is a literal token gate: without `--apply`, the command
    /// only reports. The gate exists because destructive merges are
    /// irreversible without a backup.
    #[arg(long)]
    pub apply: bool,
}

pub fn run(args: DedupeArgs, format: crate::Format) -> Result<i32> {
    let kind = linkmarks_core::SourceKind::from_cli_str(&args.source)
        .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
    if !matches!(kind, linkmarks_core::SourceKind::Chromium) {
        bail!("v1 only supports --source=chrome");
    }

    let src = linkmarks_bridge_chromium::ChromiumSource::open(&args.path)?;
    let bookmarks = src.list()?;
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
    } else {
        eprintln!(
            "(apply mode: {} canonical records, no on-disk write in v1)",
            canonical.len()
        );
    }

    if report.conflicts.is_empty() {
        Ok(crate::exit_codes::OK)
    } else {
        Ok(crate::exit_codes::DEDUPE_CONFLICTS)
    }
}
