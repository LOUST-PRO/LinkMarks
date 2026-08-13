//! Configuration for the `linkmarks-tui` browser.
//!
//! Resolution order for the default source:
//! 1. `--source=` CLI flag (handled by the CLI wrapper, not this crate)
//! 2. `LINKMARKS_TUI_DEFAULT_SOURCE` environment variable
//! 3. `all` (merge from every available source)
//!
//! XDG paths follow the same logic as `linkmarks-core::paths`.

use std::env;
use std::path::PathBuf;

/// What sources to merge when the TUI starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceSelection {
    /// All available sources on the host (browser JSON, netscape HTML, store).
    All,
    /// Chromium-family browser JSON (`Bookmarks` files).
    Chrome,
    /// Netscape-format HTML bookmark files.
    Netscape,
    /// Firefox places.sqlite (Fase 2 F3).
    Firefox,
}

impl SourceSelection {
    /// CLI/env-value string for this selection.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Chrome => "chrome",
            Self::Netscape => "netscape",
            Self::Firefox => "firefox",
        }
    }

    /// Parse a CLI/env value. Returns `None` for unknown values.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" | "" => Some(Self::All),
            "chrome" | "chromium" | "brave" | "edge" | "arc" | "vivaldi" | "opera" => {
                Some(Self::Chrome)
            }
            "netscape" | "html" => Some(Self::Netscape),
            "firefox" => Some(Self::Firefox),
            _ => None,
        }
    }
}

/// Runtime configuration for the TUI.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// Which source selection produced the merged bookmark list.
    pub selection: SourceSelection,
    /// Optional explicit path to a Netscape/HTML file.
    pub netscape_path: Option<PathBuf>,
    /// Optional explicit path to a Chromium `Bookmarks` JSON file.
    pub chromium_path: Option<PathBuf>,
    /// Optional explicit path to the local SQLite store.
    pub store_path: Option<PathBuf>,
    /// Maximum draw rate (Hz). Default 30.
    pub max_fps: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            selection: SourceSelection::All,
            netscape_path: None,
            chromium_path: None,
            store_path: None,
            max_fps: 30,
        }
    }
}

impl AppConfig {
    /// Build a config from explicit arguments. Used by the CLI wrapper.
    pub fn from_args(
        selection: SourceSelection,
        netscape_path: Option<PathBuf>,
        chromium_path: Option<PathBuf>,
        store_path: Option<PathBuf>,
    ) -> Self {
        Self {
            selection,
            netscape_path,
            chromium_path,
            store_path,
            max_fps: 30,
        }
    }

    /// Resolve from environment. Honors `LINKMARKS_TUI_DEFAULT_SOURCE`.
    #[must_use]
    pub fn from_env() -> Self {
        let selection = env::var("LINKMARKS_TUI_DEFAULT_SOURCE")
            .ok()
            .and_then(|v| SourceSelection::parse(&v))
            .unwrap_or(SourceSelection::All);
        Self::from_args(selection, None, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_all_canonical_values() {
        assert_eq!(SourceSelection::parse("all"), Some(SourceSelection::All));
        assert_eq!(
            SourceSelection::parse("chrome"),
            Some(SourceSelection::Chrome)
        );
        assert_eq!(
            SourceSelection::parse("netscape"),
            Some(SourceSelection::Netscape)
        );
        assert_eq!(
            SourceSelection::parse("firefox"),
            Some(SourceSelection::Firefox)
        );
        assert_eq!(SourceSelection::parse(""), Some(SourceSelection::All));
    }

    #[test]
    fn parse_accepts_browser_aliases() {
        for alias in ["chromium", "brave", "edge", "vivaldi", "opera"] {
            assert_eq!(
                SourceSelection::parse(alias),
                Some(SourceSelection::Chrome),
                "alias {alias}"
            );
        }
    }

    #[test]
    fn parse_rejects_unknown() {
        assert_eq!(SourceSelection::parse("bogus"), None);
        assert_eq!(SourceSelection::parse("chrome   "), Some(SourceSelection::Chrome));
    }

    #[test]
    fn from_args_sets_fps_default() {
        let c = AppConfig::from_args(SourceSelection::All, None, None, None);
        assert_eq!(c.max_fps, 30);
        assert_eq!(c.selection, SourceSelection::All);
    }
}
