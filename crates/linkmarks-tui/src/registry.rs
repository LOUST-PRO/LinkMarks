//! Source registry — dispatches reads across the available bridges and
//! the local SQLite store, deduplicating by canonical URL.

use std::path::PathBuf;

use linkmarks_core::model::Bookmark;
use linkmarks_core::traits::BookmarkSource;

use crate::config::SourceSelection;

/// A dispatchable source. Every entry knows how to materialize a
/// `Vec<Bookmark>` (either from a bridge or from the SQLite store).
#[derive(Debug, Clone)]
pub enum Source {
    /// Chromium-family browser JSON. Constructed lazily from the path.
    Chromium(PathBuf),
    /// Netscape HTML file. Constructed lazily from the path.
    Netscape(PathBuf),
    /// Local SQLite store.
    Store(PathBuf),
}

impl Source {
    /// Read all bookmarks from this source. Errors are non-fatal: the
    /// registry collects them and proceeds with the rest.
    pub fn list(&self) -> Result<Vec<Bookmark>, String> {
        match self {
            Self::Chromium(path) => {
                let src = linkmarks_bridge_chromium::ChromiumSource::open(path)
                    .map_err(|e| format!("open chromium {}: {e}", path.display()))?;
                src.list().map_err(|e| format!("list chromium: {e}"))
            }
            Self::Netscape(path) => {
                let src = linkmarks_bridge_netscape::NetscapeSource::open(path)
                    .map_err(|e| format!("open netscape {}: {e}", path.display()))?;
                src.list().map_err(|e| format!("list netscape: {e}"))
            }
            Self::Store(path) => {
                let s = linkmarks_core::store::open(path)
                    .map_err(|e| format!("open store {}: {e}", path.display()))?;
                let mut all = Vec::new();
                let mut offset = 0;
                loop {
                    let page = s
                        .list(500, offset)
                        .map_err(|e| format!("store list: {e}"))?;
                    if page.is_empty() {
                        break;
                    }
                    let more = page.len();
                    all.extend(page);
                    offset += more;
                    if more < 500 {
                        break;
                    }
                }
                Ok(all)
            }
        }
    }

    /// A short label for the status bar.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Chromium(p) => format!("chrome:{}", short_path(p)),
            Self::Netscape(p) => format!("netscape:{}", short_path(p)),
            Self::Store(p) => format!("store:{}", short_path(p)),
        }
    }
}

/// Registry wrapper: holds the resolved sources and per-source errors.
#[derive(Debug, Clone)]
pub struct SourceRegistry {
    /// Active sources. The TUI reads from these and merges.
    pub sources: Vec<Source>,
    /// Read errors collected during the most recent `load`. The TUI
    /// surfaces these in the status bar.
    pub errors: Vec<String>,
}

impl SourceRegistry {
    /// Build a registry from the CLI args + default discovery.
    pub fn resolve(
        selection: SourceSelection,
        netscape_path: Option<PathBuf>,
        chromium_path: Option<PathBuf>,
        store_path: Option<PathBuf>,
    ) -> Self {
        let mut sources = Vec::new();

        let store_path = store_path
            .or_else(|| Some(linkmarks_core::paths::linkmarks_store_path()))
            .filter(|p| p.exists());
        if let Some(p) = store_path {
            sources.push(Source::Store(p));
        }

        match selection {
            SourceSelection::All => {
                push_chromium_defaults(&mut sources, chromium_path);
                push_netscape_defaults(&mut sources, netscape_path);
            }
            SourceSelection::Chrome => {
                push_chromium_defaults(&mut sources, chromium_path);
            }
            SourceSelection::Netscape => {
                push_netscape_defaults(&mut sources, netscape_path);
            }
            SourceSelection::Firefox => {
                // Firefox is not yet wired into the TUI registry.
                // Selecting Firefox explicitly produces an empty load
                // (the store, if present, is still merged).
            }
        }

        Self {
            sources,
            errors: Vec::new(),
        }
    }

    /// Read all sources, merge, and deduplicate by canonical URL.
    pub fn load_all(&mut self) -> Vec<Bookmark> {
        let mut errors = Vec::new();
        let mut all: Vec<Bookmark> = Vec::new();
        for src in &self.sources {
            match src.list() {
                Ok(mut items) => all.append(&mut items),
                Err(e) => errors.push(format!("{}: {}", src.label(), e)),
            }
        }
        self.errors = errors;

        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        all.retain(|b| seen.insert(b.canonical_url.clone()));
        all.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        all
    }

    /// Build a one-line summary of the active sources.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = self.sources.iter().map(Source::label).collect();
        if !self.errors.is_empty() {
            parts.push(format!("{} error(s)", self.errors.len()));
        }
        if parts.is_empty() {
            "no sources (run `linkmarks init` or pass --source=chrome|netscape)".to_string()
        } else {
            parts.join(" | ")
        }
    }
}

fn push_chromium_defaults(sources: &mut Vec<Source>, override_path: Option<PathBuf>) {
    if let Some(p) = override_path {
        if p.exists() {
            sources.push(Source::Chromium(p));
        }
        return;
    }
    let candidates = linkmarks_bridge_chromium::discover_default_paths();
    for (_kind, p) in candidates {
        sources.push(Source::Chromium(p));
    }
}

fn push_netscape_defaults(sources: &mut Vec<Source>, override_path: Option<PathBuf>) {
    if let Some(p) = override_path {
        if p.exists() {
            sources.push(Source::Netscape(p));
        }
        return;
    }
    let candidates = linkmarks_bridge_netscape::discover_default_paths();
    for (_kind, p) in candidates {
        sources.push(Source::Netscape(p));
    }
}

fn short_path(p: &std::path::Path) -> String {
    let s = p.to_string_lossy();
    if s.len() <= 48 {
        s.to_string()
    } else {
        let start = &s[..24];
        let end = p
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        if end.is_empty() {
            s.to_string()
        } else {
            format!("{start}...{end}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_empty_when_no_sources_available() {
        let r = SourceRegistry::resolve(
            SourceSelection::All,
            Some(PathBuf::from("/nonexistent/netscape.html")),
            Some(PathBuf::from("/nonexistent/Bookmarks")),
            Some(PathBuf::from("/nonexistent/store.db")),
        );
        assert!(r.sources.is_empty());
    }

    #[test]
    fn summary_when_empty() {
        // Construct a registry that has no sources at all. The
        // `resolve` helper auto-discovers browsers on the host, so
        // we use a firefox-only selection (which is a no-op until the
        // firefox bridge is wired) plus a non-existent store path.
        let mut r = SourceRegistry::resolve(
            SourceSelection::Firefox,
            None,
            None,
            Some(PathBuf::from("/nonexistent/store.db")),
        );
        // Force empty in case the host has a chromium profile.
        r.sources.clear();
        assert!(r.summary().contains("no sources"));
    }
}
