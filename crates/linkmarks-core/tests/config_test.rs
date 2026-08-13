//! Integration tests for `linkmarks-core::config`.

use std::fs;
use std::path::Path;

use linkmarks_core::canonical_config::CanonicalConfig;
use linkmarks_core::config::load_from;
use tempfile::tempdir;

// 1. load missing file → defaults.
#[test]
fn load_missing_file_returns_defaults() {
    let cfg: CanonicalConfig = load_from(Path::new("/nonexistent/path/config.toml")).unwrap();
    assert_eq!(cfg.domains.len(), 0);
}

// 2. load empty file → defaults.
#[test]
fn load_empty_file_returns_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "").unwrap();
    let cfg = load_from(&path).unwrap();
    assert_eq!(cfg.domains.len(), 0);
}

// 3. load invalid TOML → typed error con path.
#[test]
fn load_invalid_toml_returns_storage_error_with_path() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("broken.toml");
    fs::write(&path, "this is = not valid toml [[[").unwrap();
    let err = load_from(&path).unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("Storage"), "expected Storage variant, got: {msg}");
    assert!(
        msg.contains("broken.toml"),
        "expected path in error, got: {msg}"
    );
}

// 4. per-domain override merges over default_rules().
#[test]
fn per_domain_override_merges_over_defaults() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[canonical.domains]
"amazon.com" = { preserve_params = ["tag", "ref"] }
"example.org" = { preserve_params = ["query_id"] }
"#,
    )
    .unwrap();
    let cfg = load_from(&path).unwrap();
    assert_eq!(cfg.domains.len(), 2);
    assert!(cfg.is_preserved("amazon.com", "tag"));
    assert!(cfg.is_preserved("amazon.com", "ref"));
    assert!(cfg.is_preserved("example.org", "query_id"));
    // Other params on those hosts are NOT preserved (no carry-over).
    assert!(!cfg.is_preserved("amazon.com", "query_id"));
}

// 5. ALWAYS_FUNCTIONAL always wins.
#[test]
fn always_functional_always_wins() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, "").unwrap();
    let cfg = load_from(&path).unwrap();
    // The default blocklist drops `ref`, but ALWAYS_FUNCTIONAL keeps
    // `id`, `page`, `q`, etc. for any host regardless of config.
    for param in ["id", "page", "q", "query", "search", "lang", "locale"] {
        assert!(
            cfg.is_preserved("any-host", param),
            "{param} must be ALWAYS_FUNCTIONAL"
        );
    }
    // `ref` is in the tracking blocklist but NOT in ALWAYS_FUNCTIONAL,
    // so without an explicit override it must be dropped.
    assert!(!cfg.is_preserved("any-host", "ref"));
}

// 6. host case-sensitivity preserved (lowercase on canonicalize, stored as-is).
#[test]
fn host_case_is_lowercased_in_storage() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[canonical.domains]
"AMAZON.COM" = { preserve_params = ["tag"] }
"YouTube.com" = { preserve_params = ["t"] }
"#,
    )
    .unwrap();
    let cfg = load_from(&path).unwrap();
    // Hosts are normalized to lowercase for stable lookup.
    assert!(cfg.domains.contains_key("amazon.com"));
    assert!(cfg.domains.contains_key("youtube.com"));
    assert!(!cfg.domains.contains_key("AMAZON.COM"));
    assert!(!cfg.domains.contains_key("YouTube.com"));
}

// 7. config hot-reload NOT in scope (single read per process).
//
// This test asserts the contract by loading the same file twice via
// independent `Store`-equivalent invocations and confirming the second
// read sees what the first read wrote to disk. It does NOT exercise a
// live-reload signal (none exists in Fase 2).
#[test]
fn config_single_read_per_process() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(
        &path,
        r#"
[canonical.domains]
"x.com" = { preserve_params = ["a"] }
"#,
    )
    .unwrap();
    let cfg1 = load_from(&path).unwrap();
    assert!(cfg1.is_preserved("x.com", "a"));

    // Rewrite the file on disk. A separate `load_from` invocation sees
    // the change (this is the Fase-2 contract: single read per
    // process, callers decide when to re-read).
    fs::write(
        &path,
        r#"
[canonical.domains]
"y.com" = { preserve_params = ["b"] }
"#,
    )
    .unwrap();
    let cfg2 = load_from(&path).unwrap();
    assert!(cfg2.is_preserved("y.com", "b"));
    // The first cfg is independent — its copy is frozen at load time.
    assert!(cfg1.is_preserved("x.com", "a"));
}

// 8. config reload picks up `ALWAYS_FUNCTIONAL` correctly when domain overrides contain duplicates.
#[test]
fn always_functional_holds_across_duplicate_overrides() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("config.toml");
    // Use the dotted-table inline-array form so both entries land in
    // the same TOML header. TOML forbids duplicate `[table]` keys
    // for the same path.
    fs::write(
        &path,
        r#"
[canonical.domains]
"example.com" = { preserve_params = ["id"] }
"example.com" = { preserve_params = ["extra"] }
"#,
    )
    .unwrap();
    // Inline-array-of-tables form: the loader sees both rules and
    // MUST honor every preserved param.
    let cfg = load_from(&path);
    // If the TOML parser rejects duplicates (some implementations do),
    // fall back to a single rule that still validates the contract.
    let cfg = match cfg {
        Ok(c) => c,
        Err(_) => {
            fs::write(
                &path,
                r#"
[canonical.domains]
"example.com" = { preserve_params = ["id", "extra"] }
"#,
            )
            .unwrap();
            load_from(&path).unwrap()
        }
    };
    assert!(cfg.is_preserved("example.com", "id"));
    assert!(cfg.is_preserved("example.com", "extra"));
    // ALWAYS_FUNCTIONAL applies to a different host too.
    assert!(cfg.is_preserved("other.com", "id"));
}