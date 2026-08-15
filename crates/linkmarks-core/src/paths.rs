//! XDG-aware filesystem paths for LinkMarks.
//!
//! All defaults follow the XDG Base Directory specification:
//! - Data: `${XDG_DATA_HOME:-~/.local/share}/linkmarks/`
//! - Config: `${XDG_CONFIG_HOME:-~/.config}/linkmarks/`
//!
//! On macOS / Windows the `dirs` crate falls back to `~/Library/Application Support/linkmarks`
//! and `%APPDATA%\linkmarks` respectively. The CLI exposes `--data-dir`,
//! `--store`, and `--config` flags to override these defaults at runtime.

use std::path::{Path, PathBuf};

/// Application name used as the XDG leaf directory.
pub const APP_DIR: &str = "linkmarks";

/// Store filename inside the data directory.
pub const STORE_FILENAME: &str = "store.db";

/// Config filename inside the config directory.
pub const CONFIG_FILENAME: &str = "config.toml";

/// Returns the data directory (`${XDG_DATA_HOME:-~/.local/share}/linkmarks`).
///
/// The directory is **not** created — callers should use
/// [`ensure_data_dir`] when they need write access.
#[must_use]
pub fn linkmarks_data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
        let trimmed = xdg.to_string_lossy().trim().to_string();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join(APP_DIR);
        }
    }
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".local/share"));
    base.join(APP_DIR)
}

/// Returns the config directory (`${XDG_CONFIG_HOME:-~/.config}/linkmarks`).
///
/// The directory is **not** created — callers should use
/// [`ensure_config_dir`] when they need write access.
#[must_use]
pub fn linkmarks_config_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let trimmed = xdg.to_string_lossy().trim().to_string();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join(APP_DIR);
        }
    }
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"));
    base.join(APP_DIR)
}

/// Returns the default SQLite store path (`<data_dir>/store.db`).
#[must_use]
pub fn linkmarks_store_path() -> PathBuf {
    linkmarks_data_dir().join(STORE_FILENAME)
}

/// Returns the default config file path (`<config_dir>/config.toml`).
#[must_use]
pub fn linkmarks_config_path() -> PathBuf {
    linkmarks_config_dir().join(CONFIG_FILENAME)
}

/// Create the data directory (and parents) if it does not exist.
pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = linkmarks_data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Create the config directory (and parents) if it does not exist.
pub fn ensure_config_dir() -> std::io::Result<PathBuf> {
    let dir = linkmarks_config_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Convenience helper: returns the default config directory as `&str` for
/// error messages; returns the literal string when the path cannot be
/// represented.
#[must_use]
pub fn default_data_dir_display() -> String {
    linkmarks_data_dir().to_string_lossy().into_owned()
}

/// Convenience helper: returns the default config directory as `String` for
/// error messages; returns the literal string when the path cannot be
/// represented.
#[must_use]
pub fn default_config_dir_display() -> String {
    linkmarks_config_dir().to_string_lossy().into_owned()
}

/// Returns whether the path lives under the configured data directory.
#[must_use]
pub fn is_inside_data_dir(path: &Path) -> bool {
    path.starts_with(linkmarks_data_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_path_lives_under_data_dir() {
        let store = linkmarks_store_path();
        assert!(store.ends_with(STORE_FILENAME));
        assert!(is_inside_data_dir(&store));
    }

    #[test]
    fn config_path_lives_under_config_dir() {
        let cfg = linkmarks_config_path();
        assert!(cfg.ends_with(CONFIG_FILENAME));
        assert!(cfg.starts_with(linkmarks_config_dir()));
    }

    #[test]
    fn data_and_config_dirs_are_distinct() {
        assert_ne!(linkmarks_data_dir(), linkmarks_config_dir());
    }

    #[test]
    fn ensure_data_dir_is_idempotent() {
        let dir = ensure_data_dir().expect("create data dir");
        assert!(dir.is_dir());
        // Second call must be a no-op and still succeed.
        let again = ensure_data_dir().expect("idempotent create");
        assert_eq!(dir, again);
    }

    #[test]
    fn ensure_config_dir_is_idempotent() {
        let dir = ensure_config_dir().expect("create config dir");
        assert!(dir.is_dir());
        let again = ensure_config_dir().expect("idempotent create");
        assert_eq!(dir, again);
    }
}
