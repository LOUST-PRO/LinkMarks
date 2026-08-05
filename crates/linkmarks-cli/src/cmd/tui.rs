//! `linkmarks tui` — launch the interactive terminal browser.

use crate::Paths;
use anyhow::Result;
use clap::Args;
use linkmarks_tui::{run as run_tui, AppConfig, SourceRegistry, SourceSelection};
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct TuiArgs {
    /// Source to read from. `all` (default) merges from every
    /// available source. `chrome`, `netscape`, and `firefox` filter
    /// to that one source.
    #[arg(long, env = "LINKMARKS_TUI_DEFAULT_SOURCE")]
    pub source: Option<String>,
    /// Optional path override for the Netscape HTML file.
    #[arg(long)]
    pub netscape_path: Option<PathBuf>,
    /// Optional path override for the Chromium `Bookmarks` JSON file.
    #[arg(long)]
    pub chromium_path: Option<PathBuf>,
}

pub fn execute(args: TuiArgs, paths: Paths) -> Result<i32> {
    let selection = parse_selection(&args.source)?;

    let registry = SourceRegistry::resolve(
        selection,
        args.netscape_path.clone(),
        args.chromium_path.clone(),
        Some(paths.store.clone()),
    );

    let config = AppConfig::from_args(
        selection,
        args.netscape_path,
        args.chromium_path,
        Some(paths.store),
    );

    let code = run_tui(registry, config).map_err(|e| anyhow::anyhow!("tui: {e}"))?;
    Ok(code)
}

fn parse_selection(raw: &Option<String>) -> Result<SourceSelection> {
    let s = raw.clone().unwrap_or_else(|| "all".to_string());
    SourceSelection::parse(&s)
        .ok_or_else(|| anyhow::anyhow!("unsupported --source={s} (try: all, chrome, netscape, firefox)"))
}
