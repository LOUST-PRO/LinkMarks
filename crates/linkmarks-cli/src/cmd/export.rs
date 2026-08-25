//! `linkmarks export` — export bookmarks to a sink format.
//!
//! Default source order:
//! 1. `--source=store`: read from the local SQLite store (default).
//! 2. `--source=chrome`: parse a Chromium JSON file.
//!
//! v2 also accepts `--source=firefox` (live `places.sqlite` or
//! `*.jsonlz4` backups) and `--source=netscape` (Netscape bookmark
//! HTML). All three path sources use [`crate::cmd::source_dispatch`].
//!
//! Sink formats:
//! - `netscape` — HTML interchange file (HTML)
//! - `json` — NDJSON, one `Bookmark` per line
//! - `chrome` — Chromium Bookmarks JSON, consumable by
//!   Vivaldi / Chrome / Edge / Brave / Arc / Opera via "Import
//!   bookmarks" UI

use crate::cmd::source_dispatch::{is_path_source, open_source, PATH_SOURCE_KINDS};
use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::store;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Netscape,
    Json,
    Chrome,
}

impl FromStr for ExportFormat {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "netscape" | "html" => Ok(Self::Netscape),
            "json" | "ndjson" => Ok(Self::Json),
            "chrome" | "chromium" => Ok(Self::Chrome),
            other => bail!("unsupported export format '{other}' (valid: netscape, json, chrome)"),
        }
    }
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    /// Output format. `netscape` (HTML), `json` (NDJSON), or
    /// `chrome` (Chromium Bookmarks JSON consumable by
    /// Vivaldi/Chrome/Edge/Brave/Arc/Opera via Import UI).
    #[arg(long, default_value = "netscape")]
    pub format: String,

    /// Source to export from. `store` reads the SQLite store
    /// (default); `chrome`, `firefox`, or `netscape` parses a
    /// browser-backed file.
    #[arg(long, default_value = "store")]
    pub source: String,

    /// Path to a source file (required for `--source=chrome|firefox|netscape`).
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Output path. `-` writes to stdout.
    #[arg(long, short = 'o', default_value = "-")]
    pub output: PathBuf,
}

pub fn run(args: ExportArgs, _format: crate::Format, paths: Paths) -> Result<i32> {
    let format: ExportFormat = args.format.parse()?;

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
        "chrome" | "firefox" | "netscape" | "html" => {
            let kind = linkmarks_core::SourceKind::from_cli_str(args.source.as_str())
                .ok_or_else(|| anyhow::anyhow!("unknown source '{}'", args.source))?;
            if !is_path_source(kind) {
                bail!(
                    "unsupported --source '{}' (try `store` or one of {:?})",
                    args.source,
                    PATH_SOURCE_KINDS
                );
            }
            let path = args
                .path
                .clone()
                .ok_or_else(|| anyhow::anyhow!("--path is required for --source={}", args.source))?;
            open_source(kind, &path)?
        }
        other => bail!(
            "unsupported --source '{other}' (try `store` or one of {:?})",
            PATH_SOURCE_KINDS
        ),
    };

    match format {
        ExportFormat::Json => {
            // NDJSON — one Bookmark per line.
            let mut out = String::new();
            for b in &bookmarks {
                out.push_str(&serde_json::to_string(b)?);
                out.push('\n');
            }
            write_output(&args.output, &out)?;
        }
        ExportFormat::Netscape => {
            let rendered = render_netscape(&bookmarks);
            write_output(&args.output, &rendered)?;
        }
        ExportFormat::Chrome => {
            write_chromium(&args.output, &bookmarks)?;
        }
    }

    Ok(crate::exit_codes::OK)
}

/// Dispatch the rendered string to stdout or to a file path.
fn write_output(output: &std::path::Path, rendered: &str) -> Result<()> {
    if output.as_os_str() == "-" {
        print!("{rendered}");
    } else {
        std::fs::write(output, rendered)
            .map_err(|e| anyhow::anyhow!("write {}: {e}", output.display()))?;
    }
    Ok(())
}

/// Emit the bookmarks as Chromium Bookmarks JSON via
/// `linkmarks-bridge-chromium`'s [`ChromiumSink`]. Atomic write if
/// the output is a file; stdout is rejected because the sink needs
/// a destination path.
fn write_chromium(
    output: &std::path::Path,
    bookmarks: &[linkmarks_core::Bookmark],
) -> Result<()> {
    if output.as_os_str() == "-" {
        bail!(
            "--format=chrome requires a file path for --output (the sink writes atomically; \
             pass e.g. --output Bookmarks.json)"
        );
    }
    let (_report, _body) = linkmarks_bridge_chromium::ChromiumSink::write_to(output, bookmarks)
        .map_err(|e| anyhow::anyhow!("chromium sink: {e}"))?;
    Ok(())
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

    #[test]
    fn export_format_parses_known_strings() {
        assert_eq!(ExportFormat::from_str("netscape").unwrap(), ExportFormat::Netscape);
        assert_eq!(ExportFormat::from_str("html").unwrap(), ExportFormat::Netscape);
        assert_eq!(ExportFormat::from_str("json").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("ndjson").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::from_str("chrome").unwrap(), ExportFormat::Chrome);
        assert_eq!(ExportFormat::from_str("chromium").unwrap(), ExportFormat::Chrome);
    }

    #[test]
    fn export_format_rejects_unknown() {
        assert!(ExportFormat::from_str("yaml").is_err());
        assert!(ExportFormat::from_str("xml").is_err());
    }
}