//! `linkmarks list` — list bookmarks deterministically.
//!
//! Default source order:
//! 1. `--source=store`: read from the local SQLite store.
//! 2. `--source=chrome`: parse a Chromium JSON file.
//!
//! `--source` is optional. When omitted, the store is used if the DB
//! exists; otherwise we fall back to the OS-typical Chrome path so the
//! CLI stays useful before `init` is run.
//!
//! v2 also accepts `--source=firefox` (live `places.sqlite` or
//! `*.jsonlz4` backups) and `--source=netscape` (Netscape bookmark
//! HTML). Both require `--path=<file>` (Firefox falls back to
//! `discover_default_paths()` if `--path` is omitted).

use crate::cmd::source_dispatch::{is_path_source, open_source, PATH_SOURCE_KINDS};
use crate::ui;
use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::model::SourceKind;
use linkmarks_core::store;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Source to list from. `store` (default when DB exists) reads from
    /// the SQLite store; `chrome`, `firefox`, or `netscape` parses a
    /// browser-backed file.
    #[arg(long)]
    pub source: Option<String>,

    /// Optional path to a source file. For `firefox`, omitted means
    /// auto-discover the first existing Firefox profile store.
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
        "chrome" | "firefox" | "netscape" | "html" => {
            let kind = linkmarks_core::SourceKind::from_cli_str(source_label.as_str())
                .ok_or_else(|| anyhow::anyhow!("unknown source '{source_label}'"))?;
            if !is_path_source(kind) {
                bail!("unsupported --source '{source_label}' (try one of {:?})", PATH_SOURCE_KINDS);
            }
            let path = match args.path.clone() {
                Some(p) => p,
                None => default_path_for(kind)?,
            };
            let bookmarks = open_source(kind, &path)?;
            let rendered = ui::render(&bookmarks, format)?;
            print!("{rendered}");
            Ok(crate::exit_codes::OK)
        }
        other => bail!(
            "unsupported --source '{other}' (try `store` or one of {:?})",
            PATH_SOURCE_KINDS
        ),
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

/// Pick a default Firefox path: scan `discover_default_paths()` for
/// the first existing `places.sqlite` or `*.jsonlz4` snapshot.
fn default_firefox_path() -> Result<PathBuf> {
    let discovered = linkmarks_bridge_firefox::discover_default_paths();
    let (_, path) = discovered
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no Firefox profile store found; pass --path"))?;
    Ok(path)
}

/// Resolve a default path for the given source kind.
fn default_path_for(kind: SourceKind) -> Result<PathBuf> {
    match kind {
        SourceKind::Chromium => Ok(default_chrome_path()),
        SourceKind::Firefox => default_firefox_path(),
        SourceKind::Netscape => Err(anyhow::anyhow!(
            "--source=netscape requires --path (no default location)"
        )),
        // `is_path_source` already rejected non-path kinds.
        _ => unreachable!("is_path_source should have filtered {kind:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_source_label_picks_store_when_initialized() {
        let store = std::env::temp_dir().join("linkmarks-test-store-does-not-exist");
        let _ = std::fs::remove_file(&store);
        assert_eq!(default_source_label(&store), "chrome");
    }

    #[test]
    fn default_chrome_path_uses_home_env() {
        let p = default_chrome_path();
        assert!(p.to_string_lossy().contains("google-chrome"));
    }
}