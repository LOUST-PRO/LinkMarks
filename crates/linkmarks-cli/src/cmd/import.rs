//! `linkmarks import` — import bookmarks from a source file.

use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ImportArgs {
    /// Source to import from. v1 supports `chrome`.
    #[arg(long, default_value = "chrome")]
    pub source: String,

    /// Path to the source file.
    #[arg(long)]
    pub path: PathBuf,
}

pub fn run(args: ImportArgs, _format: crate::Format) -> Result<i32> {
    let kind = linkmarks_core::SourceKind::from_cli_str(&args.source)
        .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
    if !matches!(kind, linkmarks_core::SourceKind::Chromium) {
        bail!("v1 only supports --source=chrome");
    }
    if !args.path.exists() {
        bail!("source file not found: {}", args.path.display());
    }

    let src = linkmarks_bridge_chromium::ChromiumSource::open(&args.path)?;
    let bookmarks = src.list()?;
    println!(
        "imported {} bookmarks from {} ({})",
        bookmarks.len(),
        args.path.display(),
        args.source
    );
    Ok(crate::exit_codes::OK)
}
