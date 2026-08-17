//! Shared parsing helpers used by bridges.
//!
//! v1 has minimal helpers — most parsing is format-specific and lives
//! in the bridge crate (e.g., Chromium JSON parsing in
//! `linkmarks-bridge-chromium`). This module is a placeholder for the
//! HTML/CSV/MIME helpers that later phases will need.

/// Decode the five named XML/HTML entities and numeric character
/// references (`&#NN;`, `&#xHH;`) commonly seen in Netscape bookmark
/// files. Other entities are passed through unchanged.
///
/// This is a v1 minimum implementation: full entity decoding is a
/// shared helper used by the Netscape bridge.
#[must_use]
pub fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        // Try to read up to ';' (bounded scan).
        let mut buf = String::new();
        let mut ended = false;
        while let Some(&next) = chars.peek() {
            if next == ';' {
                chars.next();
                ended = true;
                break;
            }
            if buf.len() > 8 {
                break;
            }
            buf.push(next);
            chars.next();
        }
        if !ended {
            out.push('&');
            out.push_str(&buf);
            continue;
        }
        match buf.as_str() {
            "amp" => out.push('&'),
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            "nbsp" => out.push('\u{00A0}'),
            other if other.starts_with('#') => {
                let body = &other[1..];
                let code =
                    if let Some(hex) = body.strip_prefix('x').or_else(|| body.strip_prefix('X')) {
                        u32::from_str_radix(hex, 16).ok()
                    } else {
                        body.parse::<u32>().ok()
                    };
                if let Some(code) = code {
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                }
            }
            _ => {
                out.push('&');
                out.push_str(&buf);
                out.push(';');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_named_entities() {
        assert_eq!(decode_html_entities("a &amp; b"), "a & b");
        assert_eq!(decode_html_entities("&lt;tag&gt;"), "<tag>");
        assert_eq!(decode_html_entities("&quot;x&quot;"), "\"x\"");
        assert_eq!(decode_html_entities("&apos;x&apos;"), "'x'");
    }

    #[test]
    fn decodes_numeric_entities() {
        assert_eq!(decode_html_entities("&#65;"), "A");
        assert_eq!(decode_html_entities("&#x41;"), "A");
    }

    #[test]
    fn passes_through_unknown() {
        assert_eq!(decode_html_entities("&unknown;"), "&unknown;");
    }

    #[test]
    fn handles_orphan_ampersand() {
        assert_eq!(decode_html_entities("a & b"), "a & b");
    }
}
