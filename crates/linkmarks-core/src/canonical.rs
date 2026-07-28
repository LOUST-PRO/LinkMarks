//! URL canonicalization for LinkMarks.
//!
//! Rules are documented in ADR-0001 (planned: `docs/decisions/0002-url-canonicalization.md`,
//! v1.0.0 ships the implementation and the rules are recorded inline
//! here until the ADR is drafted). The full ruleset lives in this
//! module's doc-comment on `canonicalize`.
//!
//! Determinism: given the same input URL, `canonicalize` returns the
//! same output bytes across runs, platforms, and processes. This is
//! the dedupe key and must be stable.

use crate::errors::CoreError;
use thiserror::Error;
use url::{Host, Url};

/// Error returned when a URL cannot be canonicalized.
#[derive(Debug, Error)]
pub enum CanonicalizeError {
    /// The URL failed to parse.
    #[error("parse error: {0}")]
    Parse(String),
    /// An IDN host that cannot be losslessly converted to ASCII.
    #[error("idn conversion failed: {0}")]
    Idn(String),
}

/// Tracking-parameter blocklist. Lowercased. Matched as a *prefix* for
/// `utm_*` and `mc_*`, and as an *exact* match for the rest.
///
/// Sources:
/// - Google Analytics (`utm_*` since ~2005)
/// - Facebook (`fbclid`)
/// - Google Ads (`gclid`)
/// - Mailchimp (`mc_eid`, `mc_cid`)
/// - Generic referrer (`ref`, `ref_src`)
pub const TRACKING_PARAMS: &[&str] = &["fbclid", "gclid", "ref", "ref_src", "mc_eid", "mc_cid"];

/// Canonicalize a URL string.
///
/// Rules (in order):
/// 1. Parse with the `url` crate. Fail on invalid input.
/// 2. Lowercase the scheme.
/// 3. Lowercase the host (ASCII). IDN hosts are converted to their
///    punycode (`xn--`) form via `url::Host::Domain` (which already
///    does this).
/// 4. Strip the default port for the scheme (`:80` for http, `:443`
///    for https).
/// 5. Drop the fragment (`#...`). Bookmarks don't carry UI state.
/// 6. Sort the remaining query parameters alphabetically by key.
/// 7. Drop tracking parameters (see `TRACKING_PARAMS`). `utm_*` and
///    `mc_*` are matched as a prefix; the rest as exact match.
/// 8. Strip the trailing slash from `path`, except when the path is
///    just `/` (the root).
/// 9. Re-serialize and return the canonical string.
///
/// The original URL is **never** modified — `Bookmark::original_url`
/// preserves it verbatim.
pub fn canonicalize(input: &str) -> Result<String, CanonicalizeError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CanonicalizeError::Parse("empty url".into()));
    }

    let mut url = Url::parse(trimmed).map_err(|e| CanonicalizeError::Parse(e.to_string()))?;

    // 2. Lowercase scheme
    let scheme = url.scheme().to_ascii_lowercase();
    url.set_scheme(&scheme)
        .map_err(|_| CanonicalizeError::Parse("invalid scheme".into()))?;

    // 3. Lowercase ASCII host; ensure IDN is punycoded.
    match url.host() {
        Some(Host::Domain(domain)) => {
            // `url` crate already normalizes IDN to ASCII when parsing
            // typical browser URLs. Force lowercase for safety.
            let lowered = domain.to_ascii_lowercase();
            url.set_host(Some(&lowered))
                .map_err(|e| CanonicalizeError::Parse(e.to_string()))?;
        }
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => {
            // Lowercase the literal representation if any hex digits
            // are present (IPv6).
            let host = url.host_str().unwrap_or("").to_ascii_lowercase();
            url.set_host(Some(&host))
                .map_err(|e| CanonicalizeError::Parse(e.to_string()))?;
        }
        None => {
            return Err(CanonicalizeError::Parse("missing host".into()));
        }
    }

    // 4. Strip default port
    let default_port: Option<u16> = match scheme.as_str() {
        "http" => Some(80),
        "https" => Some(443),
        "ftp" => Some(21),
        _ => None,
    };
    if let (Some(port), Some(default)) = (url.port(), default_port) {
        if port == default {
            let _ = url.set_port(None);
        }
    }

    // 5. Drop fragment
    url.set_fragment(None);

    // 6 + 7. Filter and sort query parameters.
    let mut kept: Vec<(String, String)> = url
        .query_pairs()
        .filter_map(|(k, v)| {
            let key_lc = k.to_ascii_lowercase();
            if is_tracking(&key_lc) {
                None
            } else {
                Some((key_lc, v.into_owned()))
            }
        })
        .collect();
    if kept.is_empty() {
        // Clear any trailing `?` left by the `url` crate after we
        // dropped all params via `set_fragment` / filtering.
        url.set_query(None);
    } else {
        kept.sort();
        // Stable sort preserves values for duplicate keys; downstream
        // consumers should treat duplicate keys as a parser quirk.
        let pairs: Vec<(String, String)> = kept;
        url.query_pairs_mut().clear();
        for (k, v) in &pairs {
            url.query_pairs_mut().append_pair(k, v);
        }
    }

    // 8. Strip trailing slash from non-root paths.
    let path = url.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        let trimmed_path = path.trim_end_matches('/').to_string();
        url.set_path(&trimmed_path);
    }

    // 9. Re-serialize. `url::Url::as_str` returns the canonical
    // serialization that matches our rules.
    let canonical = url.as_str().to_string();

    // sanity check: parse should round-trip
    Url::parse(&canonical).map_err(|e| CanonicalizeError::Parse(e.to_string()))?;

    Ok(canonical)
}

/// Returns `true` if the given (lowercased) query parameter name is
/// a known tracking parameter.
///
/// `utm_*` and `mc_*` match by prefix. The rest match exactly.
#[must_use]
pub fn is_tracking(key_lc: &str) -> bool {
    if key_lc.starts_with("utm_") || key_lc.starts_with("mc_") {
        return true;
    }
    TRACKING_PARAMS.contains(&key_lc)
}

/// Convenience wrapper: canonicalize and map errors into `CoreError`.
pub fn canonicalize_for_core(input: &str) -> Result<String, CoreError> {
    canonicalize(input).map_err(|e| CoreError::Canonicalize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_scheme_and_host() {
        let out = canonicalize("HTTPS://Example.COM/path").unwrap();
        assert_eq!(out, "https://example.com/path");
    }

    #[test]
    fn strips_default_port_http() {
        let out = canonicalize("http://example.com:80/x").unwrap();
        assert_eq!(out, "http://example.com/x");
    }

    #[test]
    fn strips_default_port_https() {
        let out = canonicalize("https://example.com:443/x").unwrap();
        assert_eq!(out, "https://example.com/x");
    }

    #[test]
    fn keeps_non_default_port() {
        let out = canonicalize("http://example.com:8080/x").unwrap();
        assert_eq!(out, "http://example.com:8080/x");
    }

    #[test]
    fn sorts_query_params() {
        let out = canonicalize("https://example.com/p?b=2&a=1&c=3").unwrap();
        assert_eq!(out, "https://example.com/p?a=1&b=2&c=3");
    }

    #[test]
    fn drops_utm_tracking_params() {
        let out = canonicalize("https://example.com/p?utm_source=x&id=42&utm_campaign=y").unwrap();
        assert_eq!(out, "https://example.com/p?id=42");
    }

    #[test]
    fn drops_known_tracking_params() {
        let cases = [
            ("https://example.com/p?fbclid=x", "https://example.com/p"),
            ("https://example.com/p?gclid=x", "https://example.com/p"),
            ("https://example.com/p?ref=x", "https://example.com/p"),
            ("https://example.com/p?ref_src=x", "https://example.com/p"),
            ("https://example.com/p?mc_eid=x", "https://example.com/p"),
            ("https://example.com/p?mc_cid=x", "https://example.com/p"),
        ];
        for (input, expected) in cases {
            assert_eq!(canonicalize(input).unwrap(), expected, "input={input}");
        }
    }

    #[test]
    fn strips_fragment() {
        let out = canonicalize("https://example.com/p#section-1").unwrap();
        assert_eq!(out, "https://example.com/p");
    }

    #[test]
    fn strips_trailing_slash_from_non_root_path() {
        assert_eq!(
            canonicalize("https://example.com/foo/").unwrap(),
            "https://example.com/foo"
        );
    }

    #[test]
    fn keeps_root_slash() {
        assert_eq!(
            canonicalize("https://example.com/").unwrap(),
            "https://example.com/"
        );
    }

    #[test]
    fn is_deterministic_across_runs() {
        let inputs = [
            "https://Example.com/PATH/?utm_source=a&b=2&a=1#frag",
            "HTTPS://example.com:443/path/?ref=x&c=3&b=2",
        ];
        for input in inputs {
            let a = canonicalize(input).unwrap();
            let b = canonicalize(input).unwrap();
            assert_eq!(a, b);
        }
    }

    #[test]
    fn invalid_url_errors() {
        assert!(canonicalize("not a url").is_err());
        assert!(canonicalize("").is_err());
        assert!(canonicalize("   ").is_err());
    }

    #[test]
    fn idn_host_lowercased_to_punycode() {
        let out = canonicalize("https://xn--bcher-kva.example/p").unwrap();
        assert_eq!(out, "https://xn--bcher-kva.example/p");
    }

    #[test]
    fn is_tracking_matches_prefix_and_exact() {
        assert!(is_tracking("utm_source"));
        assert!(is_tracking("utm_medium"));
        assert!(is_tracking("mc_eid"));
        assert!(is_tracking("fbclid"));
        assert!(!is_tracking("id"));
        assert!(!is_tracking("page"));
    }
}
