//! `linkmarks list` — list bookmarks deterministically.
//!
//! Default source order:
//! 1. `--source=store`: read from the local SQLite store.
//! 2. `--source=chrome`: parse a Chromium JSON file.
//!
//! `--source` is optional. When omitted, the store is used if the DB
//! exists; otherwise we fall back to the OS-typical Chrome path so the
//! CLI stays useful before `init` is run.

use crate::ui;
use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::store;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Source to list from. `store` (default when DB exists) reads from
    /// the SQLite store; `chrome` parses a Chromium JSON file.
    #[arg(long)]
    pub source: Option<String>,

    /// Optional path to a source file (for `chrome`). Defaults to the
    /// OS-typical location for the chosen source.
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Page size for the store source. Defaults to 100.
    #[arg(long, default_value = "100")]
    pub limit: usize,

    /// Offset for pagination. Defaults to 0.
    #[arg(long, default_value = "0")]
    pub offset: usize,
}

pub fn run(args: ListArgs, format: crate::Format, paths: Paths) -> Result<i32> {
    let source_label = args
        .source
        .clone()
        .unwrap_or_else(|| default_source_label(&paths.store).to_string());

    match source_label.as_str() {
        "store" => {
            if !paths.store.exists() {
                bail!(
                    "store not found at {}; run `linkmarks init` first",
                    paths.store.display()
                );
            }
            let s = store::open(&paths.store)?;
            let bookmarks = s.list(args.limit.max(1), args.offset)?;
            let rendered = ui::render(&bookmarks, format)?;
            print!("{rendered}");
            Ok(crate::exit_codes::OK)
        }
        "chrome" => {
            let kind = linkmarks_core::SourceKind::from_cli_str("chrome")
                .ok_or_else(|| anyhow::anyhow!("unknown source 'chrome'"))?;
            if !matches!(kind, linkmarks_core::SourceKind::Chromium) {
                bail!("v1 only supports --source=chrome");
            }
            let path = args.path.clone().unwrap_or_else(default_chrome_path);
            let src = linkmarks_bridge_chromium::ChromiumSource::open(&path)?;
            let bookmarks = src.list()?;
            let rendered = ui::render(&bookmarks, format)?;
            print!("{rendered}");
            Ok(crate::exit_codes::OK)
        }
        other => bail!("unsupported --source '{other}' (try `store` or `chrome`)"),
    }
}

/// Decide the default source label: `store` if the DB exists,
/// `chrome` otherwise. The store is preferred once `init` has run.
fn default_source_label(store_path: &std::path::Path) -> &'static str {
    if store_path.exists() {
        "store"
    } else {
        "chrome"
    }
}

fn default_chrome_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/google-chrome/Default/Bookmarks")
}
