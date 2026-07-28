//! Integration tests for `linkmarks-core::canonical`.
//!
//! Covers the rules documented in `canonical.rs`. Add a case here
//! when introducing a new rule or when a bug is reported.

use linkmarks_core::canonicalize;

#[test]
fn lowercase_scheme_and_host() {
    assert_eq!(
        canonicalize("HTTPS://Example.COM/Path").unwrap(),
        "https://example.com/Path"
    );
}

#[test]
fn strip_default_port_https() {
    assert_eq!(
        canonicalize("https://example.com:443/x").unwrap(),
        "https://example.com/x"
    );
}

#[test]
fn sort_query_params() {
    assert_eq!(
        canonicalize("https://example.com/p?b=2&a=1").unwrap(),
        "https://example.com/p?a=1&b=2"
    );
}

#[test]
fn drop_utm_prefix() {
    assert_eq!(
        canonicalize("https://example.com/p?utm_source=x&id=42").unwrap(),
        "https://example.com/p?id=42"
    );
}

#[test]
fn drop_fbclid_and_gclid() {
    assert_eq!(
        canonicalize("https://example.com/p?fbclid=x&gclid=y&id=1").unwrap(),
        "https://example.com/p?id=1"
    );
}

#[test]
fn drop_fragment() {
    assert_eq!(
        canonicalize("https://example.com/p#section-1").unwrap(),
        "https://example.com/p"
    );
}

#[test]
fn strip_trailing_slash_non_root() {
    assert_eq!(
        canonicalize("https://example.com/foo/").unwrap(),
        "https://example.com/foo"
    );
}

#[test]
fn keep_root_slash() {
    assert_eq!(
        canonicalize("https://example.com/").unwrap(),
        "https://example.com/"
    );
}

#[test]
fn round_trip_is_stable() {
    // Two semantically-equivalent inputs collapse to the same canonical form.
    let a = canonicalize("HTTPS://Example.com/p/?utm_source=x&id=42#frag").unwrap();
    let b = canonicalize("https://example.com/p?id=42").unwrap();
    assert_eq!(a, b);
}
