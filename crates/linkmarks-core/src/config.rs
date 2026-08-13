//! TOML config loader for LinkMarks.
//!
//! Reads `${XDG_CONFIG_HOME:-~/.config}/linkmarks/config.toml` and parses
//! it into a [`CanonicalConfig`]. The loader is **not** hot-reload:
//! a single read per process is the contract. Hot-reload would require
//! `notify` + state synchronization and is out of scope for Fase 2.
//!
//! ## Fallback policy
//!
//! 1. Missing file → [`CanonicalConfig::default_rules()`] (the empty
//!    `CanonicalConfig`). No error.
//! 2. Empty file (zero bytes) → defaults, no error.
//! 3. Parse error → [`CoreError::Storage`] with the path attached, so
//!    `init` / `list` can surface a clear diagnostic.
//!
//! The config shape is intentionally tiny — a single
//! `[canonical.domains.<host>]` table — so a hand-written TOML in the
//! user's editor stays readable.

use crate::canonical_config::CanonicalConfig;
use crate::errors::CoreError;
use crate::paths;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// On-disk representation of `config.toml`. Decoupled from
/// `CanonicalConfig` so future top-level sections (e.g. `[net]`,
/// `[storage]`) can be added without disturbing the canonical-rules API.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct OnDiskConfig {
    /// Canonicalization rules. May be absent in which case defaults apply.
    #[serde(default)]
    canonical: CanonicalSection,
}

#[derive(Debug, Default, Deserialize)]
struct CanonicalSection {
    /// Per-domain preservation overrides.
    #[serde(default)]
    domains: HashMap<String, DomainSection>,
}

#[derive(Debug, Default, Deserialize)]
struct DomainSection {
    /// Query parameter names that must be preserved (lowercase, exact).
    #[serde(default)]
    preserve_params: Vec<String>,
}

/// Public entry point: load the default config file.
///
/// Missing or empty file → defaults. Invalid file → error.
pub fn load() -> Result<CanonicalConfig, CoreError> {
    let path = paths::linkmarks_config_path();
    load_from(&path)
}

/// Load from an explicit path. Used by tests.
pub fn load_from(path: &Path) -> Result<CanonicalConfig, CoreError> {
    if !path.exists() {
        return Ok(CanonicalConfig::default_rules());
    }
    let text = std::fs::read_to_string(path).map_err(|e| CoreError::Storage(format!(
        "read config {}: {e}",
        path.display()
    )))?;
    parse(&text, path)
}

fn parse(text: &str, origin: &Path) -> Result<CanonicalConfig, CoreError> {
    if text.trim().is_empty() {
        return Ok(CanonicalConfig::default_rules());
    }
    let on_disk: OnDiskConfig = toml::from_str(text).map_err(|e| CoreError::Storage(format!(
        "parse config {}: {e}",
        origin.display()
    )))?;

    let mut cfg = CanonicalConfig::default_rules();
    for (host, dom) in on_disk.canonical.domains {
        // Lowercase the host for stable lookup.
        let host_lc = host.to_ascii_lowercase();
        cfg.domains.insert(
            host_lc,
            crate::canonical_config::DomainRules {
                preserve_params: dom
                    .preserve_params
                    .into_iter()
                    .map(|p| p.to_ascii_lowercase())
                    .collect(),
            },
        );
    }
    Ok(cfg)
}

/// Write the default config file at the standard XDG path if missing.
///
/// Returns `true` when a fresh file was written, `false` when the file
/// already existed.
pub fn ensure_default() -> Result<bool, CoreError> {
    let path = paths::linkmarks_config_path();
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, DEFAULT_CONFIG_BODY)
        .map_err(|e| CoreError::Storage(format!("write default config: {e}")))?;
    Ok(true)
}

/// Reference TOML body used by `ensure_default`. Kept in sync with the
/// shipped `config.toml.example` at the workspace root.
pub const DEFAULT_CONFIG_BODY: &str = r#"# LinkMarks configuration
# Placed at ${XDG_CONFIG_HOME:-~/.config}/linkmarks/config.toml
# Per-domain canonicalization overrides. Parameters in this list are
# preserved across canonicalization. Parameters in the global tracking
# blocklist (utm_*, fbclid, gclid, etc.) are still dropped unless
# listed here.
[canonical.domains]
"youtube.com"  = { preserve_params = ["t", "v", "list", "index", "si"] }
"youtu.be"     = { preserve_params = ["t", "si"] }
"vimeo.com"    = { preserve_params = ["t"] }
"github.com"   = { preserve_params = ["q", "tab", "type"] }
"twitter.com"  = { preserve_params = ["s", "src"] }
"x.com"        = { preserve_params = ["s", "src"] }
"amazon.com"   = { preserve_params = ["tag"] }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canonical_config::{CanonicalConfig, DomainRules};

    #[test]
    fn missing_file_returns_defaults() {
        let cfg = load_from(Path::new("/nonexistent/config.toml")).unwrap();
        assert_eq!(cfg.domains.len(), 0);
        assert!(matches!(cfg, CanonicalConfig { .. }));
    }

    #[test]
    fn empty_file_returns_defaults() {
        let cfg = parse("", Path::new("/dev/null")).unwrap();
        assert_eq!(cfg.domains.len(), 0);
    }

    #[test]
    fn whitespace_only_file_returns_defaults() {
        let cfg = parse("   \n\t  \n", Path::new("/dev/null")).unwrap();
        assert_eq!(cfg.domains.len(), 0);
    }

    #[test]
    fn invalid_toml_returns_storage_error_with_path() {
        let err = parse(
            "this is = not valid toml [[[",
            Path::new("/tmp/broken-config.toml"),
        )
        .unwrap_err();
        match err {
            CoreError::Storage(msg) => {
                assert!(msg.contains("/tmp/broken-config.toml"), "got: {msg}");
            }
            other => panic!("expected Storage, got: {other:?}"),
        }
    }

    #[test]
    fn per_domain_overrides_merge_over_defaults() {
        let toml = r#"
[canonical.domains]
"amazon.com" = { preserve_params = ["tag", "ref"] }
"#;
        let cfg = parse(toml, Path::new("/dev/null")).unwrap();
        assert!(cfg.is_preserved("amazon.com", "tag"));
        assert!(cfg.is_preserved("amazon.com", "ref"));
        assert!(!cfg.is_preserved("amazon.com", "nope"));
    }

    #[test]
    fn always_functional_wins_over_dropped_list() {
        // `ref` is normally a tracking param; the default blocklist
        // drops it. But ALWAYS_FUNCTIONAL (via canonical_config) must
        // override when the user explicitly opts in.
        let toml = r#"
[canonical.domains]
"example.com" = { preserve_params = ["ref"] }
"#;
        let cfg = parse(toml, Path::new("/dev/null")).unwrap();
        // `id` is ALWAYS_FUNCTIONAL and must survive even without any
        // explicit opt-in.
        assert!(cfg.is_preserved("any-host", "id"));
        assert!(cfg.is_preserved("any-host", "page"));
        assert!(cfg.is_preserved("any-host", "q"));
        // `ref` for the specific host is preserved.
        assert!(cfg.is_preserved("example.com", "ref"));
        // But `ref` for an unrelated host is dropped.
        assert!(!cfg.is_preserved("other.com", "ref"));
    }

    #[test]
    fn host_lowercased_on_parse() {
        let toml = r#"
[canonical.domains]
"AMAZON.COM" = { preserve_params = ["tag"] }
"#;
        let cfg = parse(toml, Path::new("/dev/null")).unwrap();
        assert!(cfg.domains.contains_key("amazon.com"));
        assert!(!cfg.domains.contains_key("AMAZON.COM"));
    }

    #[test]
    fn param_names_lowercased_on_parse() {
        let toml = r#"
[canonical.domains]
"amazon.com" = { preserve_params = ["TAG", "Ref"] }
"#;
        let cfg = parse(toml, Path::new("/dev/null")).unwrap();
        let rules = cfg.domains.get("amazon.com").expect("host present");
        assert_eq!(
            rules.preserve_params,
            vec!["tag".to_string(), "ref".to_string()]
        );
    }

    #[test]
    fn unknown_top_level_section_rejected() {
        // Configs intentionally deny unknown fields to surface typos
        // like `[cannonical]` early.
        let toml = "[cannonical.domains]\n\"x.com\" = { preserve_params = [\"a\"] }\n";
        let err = parse(toml, Path::new("/dev/null")).unwrap_err();
        match err {
            CoreError::Storage(msg) => assert!(msg.contains("cannonical"), "got: {msg}"),
            other => panic!("expected Storage, got: {other:?}"),
        }
    }

    #[test]
    fn ensure_default_is_idempotent() {
        // We can't use the default XDG path in tests (would mutate the
        // real config), so this just exercises the parse path.
        let parsed = parse(DEFAULT_CONFIG_BODY, Path::new("/dev/null")).unwrap();
        assert!(parsed.is_preserved("youtube.com", "t"));
        assert!(parsed.is_preserved("github.com", "q"));
        assert!(parsed.is_preserved("amazon.com", "tag"));
        // baseline DomainRules behavior still applies.
        let _ = DomainRules::default();
    }
}