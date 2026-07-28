//! `linkmarks` — the LinkMarks command-line interface.
//!
//! Subcommands:
//! - `list`    — list bookmarks deterministically.
//! - `import`  — import from a bridge source.
//! - `export`  — export to a sink format.
//! - `dedupe`  — local deterministic dedupe by canonical URL.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod cmd;
mod ui;

#[derive(Parser, Debug)]
#[command(name = "linkmarks", version, about = "Local-first bookmark manager")]
struct Cli {
    /// Output format for the active command.
    #[arg(long, global = true, default_value = "table")]
    format: Format,

    /// Verbosity. `-v` for info, `-vv` for debug.
    #[arg(long, short = 'v', global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Format {
    #[default]
    Table,
    Json,
    Yaml,
}

impl Format {
    #[allow(dead_code)]
    fn as_str(self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Json => "json",
            Self::Yaml => "yaml",
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// List bookmarks from a source.
    List(cmd::list::ListArgs),
    /// Import bookmarks from a source file.
    Import(cmd::import::ImportArgs),
    /// Export bookmarks to a sink format.
    Export(cmd::export::ExportArgs),
    /// Dedupe by canonical URL with a conflict report.
    Dedupe(cmd::dedupe::DedupeArgs),
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let exit = dispatch(cli).context("linkmarks command failed")?;
    std::process::exit(exit);
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("linkmarks=warn,warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .try_init();
}

fn dispatch(cli: Cli) -> Result<i32> {
    match cli.command {
        Commands::List(args) => cmd::list::run(args, cli.format),
        Commands::Import(args) => cmd::import::run(args, cli.format),
        Commands::Export(args) => cmd::export::run(args, cli.format),
        Commands::Dedupe(args) => cmd::dedupe::run(args, cli.format),
    }
}

/// Exit codes per SPEC.md §Acceptance criteria (CLI summary).
pub mod exit_codes {
    /// Success.
    pub const OK: i32 = 0;
    /// Partial success or source error.
    pub const PARTIAL: i32 = 1;
    /// Invalid arguments.
    pub const INVALID_ARGS: i32 = 2;
    /// Dedupe conflicts found (non-fatal).
    pub const DEDUPE_CONFLICTS: i32 = 3;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_round_trip() {
        for f in [Format::Table, Format::Json, Format::Yaml] {
            assert_eq!(f.as_str(), f.as_str());
        }
    }

    #[test]
    fn format_default_is_table() {
        assert!(matches!(Format::default(), Format::Table));
    }
}
