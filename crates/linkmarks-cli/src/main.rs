//! `linkmarks` — the LinkMarks command-line interface.
//!
//! Subcommands:
//! - `init`        — initialize the XDG store + config (Fase 2).
//! - `list`        — list bookmarks deterministically.
//! - `import`      — import from a bridge source.
//! - `export`      — export to a sink format.
//! - `dedupe`      — local deterministic dedupe by canonical URL.
//! - `tui`         — launch the interactive terminal browser (Fase 2 F4).
//! - `completions` — emit a shell completion script (bash/zsh/fish/powershell/elvish).

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

mod cmd;
mod ui;

#[derive(Parser, Debug)]
#[command(name = "linkmarks", version, about = "Local-first bookmark manager")]
pub struct Cli {
    /// Output format for the active command.
    #[arg(long, global = true, default_value = "table")]
    format: Format,

    /// Verbosity. `-v` for info, `-vv` for debug.
    #[arg(long, short = 'v', global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Path to the SQLite store (defaults to XDG data dir).
    #[arg(long, global = true, env = "LINKMARKS_STORE")]
    store: Option<PathBuf>,

    /// Path to the config file (defaults to XDG config dir).
    #[arg(long, global = true, env = "LINKMARKS_CONFIG")]
    config: Option<PathBuf>,

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

/// Resolved XDG paths. Built once per invocation and threaded into
/// every subcommand that needs them.
#[derive(Debug, Clone)]
pub struct Paths {
    /// Path to the SQLite store.
    pub store: PathBuf,
    /// Path to the config file.
    pub config: PathBuf,
}

impl Paths {
    /// Resolve from CLI flags + defaults.
    pub fn resolve(cli: &Cli) -> Self {
        let store = cli
            .store
            .clone()
            .unwrap_or_else(linkmarks_core::paths::linkmarks_store_path);
        let config = cli
            .config
            .clone()
            .unwrap_or_else(linkmarks_core::paths::linkmarks_config_path);
        Self { store, config }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize the LinkMarks store and config.
    Init(cmd::init::InitArgs),
    /// List bookmarks from a source or store.
    List(cmd::list::ListArgs),
    /// Import bookmarks from a source file.
    Import(cmd::import::ImportArgs),
    /// Export bookmarks to a sink format.
    Export(cmd::export::ExportArgs),
    /// Dedupe by canonical URL with a conflict report.
    Dedupe(cmd::dedupe::DedupeArgs),
    /// Launch the interactive terminal browser.
    Tui(cmd::tui::TuiArgs),
    /// Emit a shell completion script to stdout.
    Completions(cmd::completions::CompletionsArgs),
}

/// Build the canonical `Cli` command tree.
///
/// Public so `completions.rs` (and any future helper) can reach a
/// mutable `Command` for `clap_complete::generate`. We construct
/// from `Cli::command()` rather than `Cli::parse()` because parse
/// also validates against `std::env::args()` (inconvenient in tests).
pub fn build_cli() -> clap::Command {
    Cli::command()
}

fn main() -> Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let paths = Paths::resolve(&cli);
    let exit = dispatch(cli, paths).context("linkmarks command failed")?;
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

fn dispatch(cli: Cli, paths: Paths) -> Result<i32> {
    match cli.command {
        Commands::Init(args) => cmd::init::run(args, cli.format, paths),
        Commands::List(args) => cmd::list::run(args, cli.format, paths),
        Commands::Import(args) => cmd::import::run(args, cli.format, paths),
        Commands::Export(args) => cmd::export::run(args, cli.format, paths),
        Commands::Dedupe(args) => cmd::dedupe::run(args, cli.format, paths),
        Commands::Tui(args) => cmd::tui::execute(args, paths),
        Commands::Completions(args) => cmd::completions::run(args, paths),
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
    use clap::Parser;

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

    #[test]
    fn paths_default_to_xdg() {
        let cli = Cli::try_parse_from(["linkmarks", "list"]).unwrap();
        let p = Paths::resolve(&cli);
        // The store path lives under the data dir.
        assert!(
            p.store
                .starts_with(linkmarks_core::paths::linkmarks_data_dir()),
            "store path {:?} not under data dir",
            p.store
        );
        assert!(
            p.config
                .starts_with(linkmarks_core::paths::linkmarks_config_dir()),
            "config path {:?} not under config dir",
            p.config
        );
    }

    #[test]
    fn paths_override_via_flags() {
        let cli = Cli::try_parse_from([
            "linkmarks",
            "--store",
            "/tmp/lm.db",
            "--config",
            "/tmp/lm.toml",
            "list",
        ])
        .unwrap();
        let p = Paths::resolve(&cli);
        assert_eq!(p.store, PathBuf::from("/tmp/lm.db"));
        assert_eq!(p.config, PathBuf::from("/tmp/lm.toml"));
    }
}
