//! `linkmarks list` — list bookmarks deterministically.

use crate::ui;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Source to list from. v1 supports `chrome`.
    #[arg(long, default_value = "chrome")]
    pub source: String,

    /// Optional path to a source file. Defaults to the OS-typical
    /// location for the chosen source.
    #[arg(long)]
    pub path: Option<PathBuf>,
}

pub fn run(args: ListArgs, format: crate::Format) -> Result<i32> {
    let kind = linkmarks_core::SourceKind::from_cli_str(&args.source)
        .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
    if !matches!(kind, linkmarks_core::SourceKind::Chromium) {
        bail!("v1 only supports --source=chrome");
    }

    let path = args.path.unwrap_or_else(default_chrome_path);
    let src = linkmarks_bridge_chromium::ChromiumSource::open(&path)?;
    let bookmarks = src.list()?;
    let rendered = ui::render(&bookmarks, format)?;
    print!("{rendered}");
    Ok(crate::exit_codes::OK)
}

fn default_chrome_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/google-chrome/Default/Bookmarks")
}
