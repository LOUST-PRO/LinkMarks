//! `linkmarks init` — initialize the LinkMarks store and config.
//!
//! What it does:
//! 1. Creates the XDG data directory (`~/.local/share/linkmarks/`).
//! 2. Creates the XDG config directory (`~/.config/linkmarks/`).
//! 3. Writes a default `config.toml` if none exists.
//! 4. Opens the store (which runs the migrator and stamps
//!    `PRAGMA user_version`).
//!
//! Idempotent: re-running is a no-op when both dirs already exist and
//! `config.toml` is present.

use anyhow::Result;
use clap::Args;
use linkmarks_core::config as core_config;
use linkmarks_core::paths;
use linkmarks_core::store;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Override the data directory (defaults to XDG).
    #[arg(long)]
    pub data_dir: Option<std::path::PathBuf>,

    /// Override the config directory (defaults to XDG).
    #[arg(long)]
    pub config_dir: Option<std::path::PathBuf>,

    /// Overwrite an existing config file with the bundled defaults.
    /// Off by default; the operator decides when to discard their
    /// hand-written rules.
    #[arg(long)]
    pub force: bool,
}

pub fn run(args: InitArgs, _format: crate::Format, paths: crate::Paths) -> Result<i32> {
    // The CLI resolves --store / --config (or LINKMARKS_STORE / LINKMARKS_CONFIG)
    // into concrete file paths. We derive the directory from the file's parent
    // so init honors the same env vars every other subcommand uses.
    let store_path = paths.store.clone();
    let cfg_path = paths.config.clone();

    let data_dir = args.data_dir.clone().unwrap_or_else(|| {
        store_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(paths::linkmarks_data_dir)
    });
    let config_dir = args.config_dir.clone().unwrap_or_else(|| {
        cfg_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(paths::linkmarks_config_dir)
    });

    // 1+2. Create directories.
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| anyhow::anyhow!("create data dir {}: {e}", data_dir.display()))?;
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| anyhow::anyhow!("create config dir {}: {e}", config_dir.display()))?;

    // 3. Default config (skip when the file exists, unless --force).
    let wrote_config = if args.force || !cfg_path.exists() {
        if args.force && cfg_path.exists() {
            // Best-effort rename to a `.bak` so the operator can recover.
            let backup = cfg_path.with_extension("toml.bak");
            let _ = std::fs::rename(&cfg_path, &backup);
        }
        std::fs::write(&cfg_path, core_config::DEFAULT_CONFIG_BODY)
            .map_err(|e| anyhow::anyhow!("write default config: {e}"))?;
        true
    } else {
        false
    };

    // 4. Open the store (runs the migrator).
    let _store = store::open(&store_path)
        .map_err(|e| anyhow::anyhow!("open store {}: {e}", store_path.display()))?;
    let _cfg = core_config::load_from(&cfg_path)
        .map_err(|e| anyhow::anyhow!("parse config {}: {e}", cfg_path.display()))?;

    println!(
        "data_dir={}\nconfig_dir={}\nstore={}\nconfig_file={}\nconfig_written={}",
        data_dir.display(),
        config_dir.display(),
        store_path.display(),
        cfg_path.display(),
        wrote_config,
    );
    Ok(crate::exit_codes::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_args_parse() {
        let _ = InitArgs {
            data_dir: None,
            config_dir: None,
            force: false,
        };
    }

    #[test]
    fn store_opens_against_arbitrary_data_dir() {
        // Smoke check: `store::open` succeeds against a fresh directory.
        let dir = tempdir().unwrap();
        let s = store::open(&dir.path().join("store.db")).unwrap();
        assert_eq!(s.count().unwrap(), 0);
    }
}
