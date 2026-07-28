//! `linkmarks export` — export bookmarks to a sink format.
//!
//! v1 supports Netscape HTML only. The implementation here emits a
//! stub that documents the format boundary; full Netscape HTML
//! serialization is in scope for the next iteration (currently the
//! canonical JSON serialization is exposed via `--format=json` for
//! round-trip testing).

use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Output format. v1 supports `netscape` and `json`.
    #[arg(long, default_value = "netscape")]
    pub format: String,

    /// Source to export from.
    #[arg(long, default_value = "chrome")]
    pub source: String,

    /// Path to a source file (Chromium JSON).
    #[arg(long)]
    pub path: PathBuf,

    /// Output path. `-` writes to stdout.
    #[arg(long, short = 'o')]
    pub output: PathBuf,
}

pub fn run(args: ExportArgs, _format: crate::Format) -> Result<i32> {
    let kind = linkmarks_core::SourceKind::from_cli_str(&args.source)
        .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
    if !matches!(kind, linkmarks_core::SourceKind::Chromium) {
        bail!("v1 only supports --source=chrome");
    }

    let src = linkmarks_bridge_chromium::ChromiumSource::open(&args.path)?;
    let bookmarks = src.list()?;

    let rendered = match args.format.as_str() {
        "json" => {
            let mut out = String::new();
            for b in &bookmarks {
                out.push_str(&serde_json::to_string(b)?);
                out.push('\n');
            }
            out
        }
        "netscape" => render_netscape(&bookmarks),
        other => bail!("unsupported export format '{other}' (v1: netscape, json)"),
    };

    if args.output.as_os_str() == "-" {
        print!("{rendered}");
    } else {
        std::fs::write(&args.output, rendered)
            .map_err(|e| anyhow::anyhow!("write {}: {e}", args.output.display()))?;
    }
    Ok(crate::exit_codes::OK)
}

fn render_netscape(bookmarks: &[linkmarks_core::Bookmark]) -> String {
    // Minimal Netscape bookmark file. Real implementation comes when
    // the Netscape bridge lands (Fase 1 polish); this keeps the
    // surface stable for downstream tooling.
    let mut out = String::new();
    out.push_str("<!DOCTYPE NETSCAPE-Bookmark-file-1>\n");
    out.push_str("<!-- This is an automatically generated file.\n");
    out.push_str("     It will be read and overwritten.\n");
    out.push_str("     DO NOT EDIT! -->\n");
    out.push_str("<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n");
    out.push_str("<TITLE>Bookmarks</TITLE>\n");
    out.push_str("<H1>Bookmarks</H1>\n");
    out.push_str("<DL><p>\n");
    for b in bookmarks {
        let add_date = b.updated_at.timestamp();
        let href = &b.original_url;
        out.push_str(&format!(
            "    <DT><A HREF=\"{href}\" ADD_DATE=\"{add_date}\">{title}</A>\n",
            href = html_escape(href),
            add_date = add_date,
            title = html_escape(&b.title),
        ));
        if let Some(desc) = &b.description {
            out.push_str(&format!("    <DD>{}\n", html_escape(desc)));
        }
    }
    out.push_str("</DL><p>\n");
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_basic() {
        assert_eq!(html_escape("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
    }
}
