//! `linkmarks export` — export bookmarks to a sink format.
//!
//! Default source order:
//! 1. `--source=store` (Fase 2): read from the local SQLite store.
//! 2. `--source=chrome` (Fase 1 legacy): parse a Chromium JSON file.
//!
//! Sink formats: `netscape` (HTML) and `json` (NDJSON, one
//! `Bookmark` per line). `--output=-` writes to stdout; any other
//! value is treated as a file path.

use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::store;
use linkmarks_core::traits::BookmarkSource;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Output format. `netscape` emits an HTML interchange file;
    /// `json` emits NDJSON.
    #[arg(long, default_value = "netscape")]
    pub format: String,

    /// Source to export from. `store` reads the SQLite store (Fase 2
    /// default); `chrome` parses a Chromium JSON file.
    #[arg(long, default_value = "store")]
    pub source: String,

    /// Path to a source file (required for `--source=chrome`).
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Output path. `-` writes to stdout.
    #[arg(long, short = 'o', default_value = "-")]
    pub output: PathBuf,
}

pub fn run(args: ExportArgs, _format: crate::Format, paths: Paths) -> Result<i32> {
    let bookmarks = match args.source.as_str() {
        "store" => {
            if !paths.store.exists() {
                bail!(
                    "store not found at {}; run `linkmarks init` first",
                    paths.store.display()
                );
            }
            let s = store::open(&paths.store)?;
            let mut all = Vec::new();
            let mut offset = 0usize;
            loop {
                let page = s.list(500, offset)?;
                if page.is_empty() {
                    break;
                }
                let page_len = page.len();
                offset += page_len;
                all.extend(page);
                if page_len < 500 {
                    break;
                }
            }
            all
        }
        "chrome" => {
            let path = args
                .path
                .clone()
                .ok_or_else(|| anyhow::anyhow!("--path is required for --source=chrome"))?;
            let src = linkmarks_bridge_chromium::ChromiumSource::open(&path)?;
            src.list()?
        }
        other => bail!("unsupported --source '{other}' (try `store` or `chrome`)"),
    };

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