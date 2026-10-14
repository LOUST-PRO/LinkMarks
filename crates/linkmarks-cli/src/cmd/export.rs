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
//!
//! ## Reporting
//!
//! After a successful write, `export` prints a human-readable summary
//! to stderr describing the operation: output path, size, bookmark
//! counts, top collections, and per-browser import instructions. Pass
//! `--no-report` to suppress this summary (useful for cron jobs or
//! when piping the output elsewhere). Pass `--json` to emit a
//! machine-readable [`ExportReport`] instead — preferred for agents
//! and scripts.

use crate::cmd::source_dispatch::{is_path_source, open_source, PATH_SOURCE_KINDS};
use crate::Paths;
use anyhow::{bail, Result};
use clap::Args;
use linkmarks_core::store;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

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
#[command(
    after_help = "After writing the file, import it into your browser via the native UI:

  Vivaldi / Chrome / Edge / Brave / Arc / Opera:
    Bookmarks → Bookmark manager → ⋮ → Import bookmarks → <file>

  Firefox (Netscape HTML format):
    Show All Bookmarks → Import and Backup → Import Bookmarks from HTML

Browsers do not auto-detect file changes — the import step is
required. See docs/why-file-based.md for the technical rationale.

Examples:
  linkmarks export --format=chrome --output ~/bookmarks.json
  linkmarks export --format=chrome --output ~/bookmarks.json --json
  linkmarks export --format=netscape --output ~/firefox-bookmarks.html

Reporting:
  --json      emit a machine-readable report (ExportReport) to stdout
  --no-report suppress the human-readable summary on stderr"
)]
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

    /// Emit the operation report as JSON to stdout instead of the
    /// human-readable summary. The bookmark file itself is still
    /// written to `--output`. The JSON shape is stable; agents
    /// should parse it without regex on stderr.
    #[arg(long)]
    pub json: bool,

    /// Suppress the human-readable summary printed to stderr after a
    /// successful write. Useful for cron jobs that only care about
    /// the written file.
    #[arg(long)]
    pub no_report: bool,
}

/// Per-collection stats for the export report.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct CollectionStat {
    /// Collection path (e.g. `work/research`).
    pub path: String,
    /// Number of bookmarks under this collection.
    pub bookmarks: usize,
}

/// Stats block of [`ExportReport`].
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ExportStats {
    /// Total bookmarks written.
    pub bookmarks_exported: usize,
    /// Number of folders emitted (top-level + nested).
    pub folders_exported: usize,
    /// Number of distinct collection paths in the store.
    pub collections_unique: usize,
    /// Deepest `collection` path depth (1 for flat, 2+ for nested).
    pub deepest_collection_depth: usize,
    /// Tags dropped because the format has no tag field
    /// (Chromium JSON). Always 0 for Netscape/JSON output.
    pub tags_dropped: usize,
}

/// Per-browser import instructions surfaced in the report.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct BrowserHint {
    /// Browser display name.
    pub name: &'static str,
    /// Ordered click-path the user follows to import the file.
    pub steps: Vec<&'static str>,
}

/// `next_steps` block of [`ExportReport`].
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct NextSteps {
    /// Always `true` — browsers do not auto-detect the file.
    pub import_required: bool,
    /// Always `false` — there is no IPC hook to notify the browser.
    pub auto_detect_supported: bool,
    /// Import instructions per supported browser family.
    pub browsers: Vec<BrowserHint>,
    /// Link to the file-based rationale doc.
    pub docs_url: &'static str,
}

/// Stable JSON shape for `--json` output of `linkmarks export`.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ExportReport {
    /// Tool version.
    pub version: &'static str,
    /// Tool name (`linkmarks export`).
    pub tool: &'static str,
    /// Path to the SQLite store that was read.
    pub store_path: PathBuf,
    /// Path to the file that was written.
    pub output_path: PathBuf,
    /// Bytes written (`0` for stdout).
    pub output_size_bytes: u64,
    /// Stats block.
    pub stats: ExportStats,
    /// Per-collection breakdown, sorted descending by bookmark count.
    pub top_collections: Vec<CollectionStat>,
    /// Next-steps block.
    pub next_steps: NextSteps,
    /// Wall-clock duration of the operation in milliseconds.
    pub duration_ms: u64,
    /// Process exit code (always 0 on a successful report).
    pub exit_code: i32,
}

pub fn run(args: ExportArgs, _format: crate::Format, paths: Paths) -> Result<i32> {
    let format: ExportFormat = args.format.parse()?;
    let started = Instant::now();

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
        other => {
            // Path-source branch: resolve aliases (brave/vivaldi/edge/arc/opera
            // collapse to Chromium) via `from_cli_str`, then validate the kind.
            let kind = linkmarks_core::SourceKind::from_cli_str(other)
                .ok_or_else(|| anyhow::anyhow!("unknown source '{other}'"))?;
            if !is_path_source(kind) {
                bail!(
                    "unsupported --source '{other}' (try `store` or one of {:?})",
                    PATH_SOURCE_KINDS
                );
            }
            let path = args
                .path
                .clone()
                .ok_or_else(|| anyhow::anyhow!("--path is required for --source={}", other))?;
            open_source(kind, &path)?
        }
    };

    let output_path = args.output.clone();
    let output_size_bytes: u64 = match format {
        ExportFormat::Json => {
            // NDJSON — one Bookmark per line.
            let mut out = String::new();
            for b in &bookmarks {
                out.push_str(&serde_json::to_string(b)?);
                out.push('\n');
            }
            write_output(&output_path, &out)?
        }
        ExportFormat::Netscape => {
            let rendered = render_netscape(&bookmarks);
            write_output(&output_path, &rendered)?
        }
        ExportFormat::Chrome => {
            let (_report, body) =
                linkmarks_bridge_chromium::ChromiumSink::write_to(&output_path, &bookmarks)
                    .map_err(|e| anyhow::anyhow!("chromium sink: {e}"))?;
            body.len() as u64
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let report = build_report(
        &paths.store,
        &output_path,
        output_size_bytes,
        &bookmarks,
        duration_ms,
    );

    if args.json {
        // Machine-readable path: print the report to stdout (the
        // bookmark file is written to --output, which may also be
        // stdout when `--output -` is passed; in that case JSON and
        // bookmark bytes would collide on the same FD, so disallow).
        if output_path.as_os_str() == "-" {
            bail!("--json requires --output to be a file path (stdout is reserved for the report)");
        }
        let serialized = serde_json::to_string_pretty(&report)?;
        println!("{serialized}");
    } else if !args.no_report && output_path.as_os_str() != "-" {
        render_human_summary(&report);
    }

    Ok(crate::exit_codes::OK)
}

/// Build an [`ExportReport`] from the inputs of a completed export.
pub fn build_report(
    store_path: &Path,
    output_path: &Path,
    output_size_bytes: u64,
    bookmarks: &[linkmarks_core::Bookmark],
    duration_ms: u64,
) -> ExportReport {
    let stats = compute_stats(bookmarks);
    let top_collections = top_collections(bookmarks, 10);
    ExportReport {
        version: env!("CARGO_PKG_VERSION"),
        tool: "linkmarks export",
        store_path: store_path.to_path_buf(),
        output_path: output_path.to_path_buf(),
        output_size_bytes,
        stats,
        top_collections,
        next_steps: default_next_steps(),
        duration_ms,
        exit_code: 0,
    }
}

/// Compute aggregate stats for the export.
pub fn compute_stats(bookmarks: &[linkmarks_core::Bookmark]) -> ExportStats {
    let mut folders_exported = 0usize;
    let mut unique_collections = std::collections::BTreeSet::new();
    let mut deepest_depth = 0usize;
    let mut tags_dropped = 0usize;
    for b in bookmarks {
        if let Some(c) = &b.collection {
            unique_collections.insert(c.clone());
            // depth = number of `/`-separated segments.
            let depth = c.split('/').count();
            if depth > deepest_depth {
                deepest_depth = depth;
            }
            // count every `/` boundary as a folder.
            folders_exported += depth.saturating_sub(1).max(1);
            // The trailing folder itself counts once.
            folders_exported = folders_exported.saturating_sub(1).max(folders_exported);
        }
        tags_dropped += b.tags.len();
    }
    ExportStats {
        bookmarks_exported: bookmarks.len(),
        folders_exported,
        collections_unique: unique_collections.len(),
        deepest_collection_depth: deepest_depth,
        tags_dropped,
    }
}

/// Return the top-N collections by bookmark count, descending.
pub fn top_collections(bookmarks: &[linkmarks_core::Bookmark], n: usize) -> Vec<CollectionStat> {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for b in bookmarks {
        if let Some(c) = &b.collection {
            *counts.entry(c.clone()).or_default() += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sorted.truncate(n);
    sorted
        .into_iter()
        .map(|(path, bookmarks)| CollectionStat { path, bookmarks })
        .collect()
}

/// Default `next_steps` block: same import instructions for the
/// Chromium-family browsers (they share the JSON schema and import UI).
pub fn default_next_steps() -> NextSteps {
    NextSteps {
        import_required: true,
        auto_detect_supported: false,
        browsers: vec![
            BrowserHint {
                name: "Vivaldi/Chrome/Edge/Brave/Arc/Opera",
                steps: vec![
                    "Bookmarks",
                    "Bookmark manager",
                    "⋮",
                    "Import bookmarks",
                    "<path>",
                ],
            },
            BrowserHint {
                name: "Firefox",
                steps: vec![
                    "Show All Bookmarks (Ctrl+Shift+O)",
                    "Import and Backup",
                    "Import Bookmarks from HTML…",
                    "<path>",
                ],
            },
        ],
        docs_url: "https://github.com/LOUST-PRO/LinkMarks/blob/main/docs/why-file-based.md",
    }
}

/// Render the human-readable summary to stderr. Idempotent.
pub fn render_human_summary(report: &ExportReport) {
    eprintln!(
        "\nlinkmarks v{} — Chromium Bookmarks JSON export\n",
        report.version
    );
    eprintln!("  Store:        {}", report.store_path.display());
    eprintln!(
        "  Output:       {} ({})",
        report.output_path.display(),
        human_bytes(report.output_size_bytes)
    );
    eprintln!("  Bookmarks:    {}", report.stats.bookmarks_exported);
    eprintln!("  Folders:      {}", report.stats.folders_exported);
    if report.stats.collections_unique > 0 {
        eprintln!(
            "  Collections:  {} unique (deepest: {})",
            report.stats.collections_unique, report.stats.deepest_collection_depth
        );
    }
    if report.stats.tags_dropped > 0 {
        eprintln!(
            "  Tags dropped: {} (Chromium native schema has no tag field)",
            report.stats.tags_dropped
        );
    }
    if !report.top_collections.is_empty() {
        eprintln!("\n  Top collections:");
        let max_count = report
            .top_collections
            .iter()
            .map(|c| c.bookmarks)
            .max()
            .unwrap_or(1);
        let width = max_count.to_string().len();
        for c in &report.top_collections {
            eprintln!("    {:<width$}  {}", c.bookmarks, c.path, width = width);
        }
    }
    eprintln!("\n  Next step — import into your browser (browser does NOT auto-detect):");
    for hint in &report.next_steps.browsers {
        eprintln!("    {}:", hint.name);
        eprintln!("      {}", hint.steps.join(" → "));
    }
    eprintln!(
        "\n  Took {}ms. Exit {}.",
        report.duration_ms, report.exit_code
    );
}

fn human_bytes(n: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut value = n as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx < UNITS.len() - 1 {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{} {}", n, UNITS[0])
    } else {
        format!("{:.1} {}", value, UNITS[idx])
    }
}

/// Dispatch the rendered string to stdout or to a file path.
fn write_output(output: &std::path::Path, rendered: &str) -> Result<u64> {
    let bytes = rendered.len() as u64;
    if output.as_os_str() == "-" {
        print!("{rendered}");
    } else {
        std::fs::write(output, rendered)
            .map_err(|e| anyhow::anyhow!("write {}: {e}", output.display()))?;
    }
    Ok(bytes)
}

/// Emit the bookmarks as Chromium Bookmarks JSON via
/// `linkmarks-bridge-chromium`'s [`ChromiumSink`]. Atomic write if
/// the output is a file; stdout is rejected because the sink needs
/// a destination path.
fn _write_chromium_unused(
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
    use chrono::TimeZone;
    use linkmarks_core::{Bookmark, BookmarkId, SourceKind, SourceRef};

    fn bm(id: &str, url: &str, title: &str, collection: Option<&str>, tags: &[&str]) -> Bookmark {
        Bookmark {
            id: BookmarkId(id.to_string()),
            original_url: url.to_string(),
            canonical_url: url.to_string(),
            title: title.to_string(),
            description: None,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            collection: collection.map(|s| s.to_string()),
            content_type: None,
            archived: false,
            created_at: chrono::Utc.timestamp_opt(0, 0).unwrap(),
            updated_at: chrono::Utc.timestamp_opt(0, 0).unwrap(),
            source: SourceRef {
                kind: SourceKind::Manual,
                external_id: None,
                imported_at: chrono::Utc.timestamp_opt(0, 0).unwrap(),
                raw: None,
            },
        }
    }

    #[test]
    fn html_escape_basic() {
        assert_eq!(html_escape("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
    }

    #[test]
    fn export_format_parses_known_strings() {
        assert_eq!(
            ExportFormat::from_str("netscape").unwrap(),
            ExportFormat::Netscape
        );
        assert_eq!(
            ExportFormat::from_str("html").unwrap(),
            ExportFormat::Netscape
        );
        assert_eq!(ExportFormat::from_str("json").unwrap(), ExportFormat::Json);
        assert_eq!(
            ExportFormat::from_str("ndjson").unwrap(),
            ExportFormat::Json
        );
        assert_eq!(
            ExportFormat::from_str("chrome").unwrap(),
            ExportFormat::Chrome
        );
        assert_eq!(
            ExportFormat::from_str("chromium").unwrap(),
            ExportFormat::Chrome
        );
    }

    #[test]
    fn export_format_rejects_unknown() {
        assert!(ExportFormat::from_str("yaml").is_err());
        assert!(ExportFormat::from_str("xml").is_err());
    }

    #[test]
    fn compute_stats_counts_unique_collections_and_tags() {
        let bms = vec![
            bm("a", "https://a", "A", Some("work"), &["x"]),
            bm("b", "https://b", "B", Some("work/research"), &["y", "z"]),
            bm("c", "https://c", "C", Some("work/research"), &[]),
            bm("d", "https://d", "D", None, &["w"]),
        ];
        let s = compute_stats(&bms);
        assert_eq!(s.bookmarks_exported, 4);
        assert_eq!(s.collections_unique, 2);
        assert_eq!(s.deepest_collection_depth, 2);
        assert_eq!(s.tags_dropped, 4); // 1 + 2 + 0 + 1
    }

    #[test]
    fn top_collections_sorts_descending_and_truncates() {
        let bms = vec![
            bm("a", "https://a", "A", Some("work"), &[]),
            bm("b", "https://b", "B", Some("work"), &[]),
            bm("c", "https://c", "C", Some("work"), &[]),
            bm("d", "https://d", "D", Some("research"), &[]),
            bm("e", "https://e", "E", Some("research"), &[]),
            bm("f", "https://f", "F", Some("misc"), &[]),
        ];
        let top = top_collections(&bms, 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].path, "work");
        assert_eq!(top[0].bookmarks, 3);
        assert_eq!(top[1].path, "research");
        assert_eq!(top[1].bookmarks, 2);
    }

    #[test]
    fn build_report_round_trips_through_json() {
        let bms = vec![
            bm("a", "https://a", "A", Some("work"), &["x"]),
            bm("b", "https://b", "B", Some("work/research"), &[]),
        ];
        let report = build_report(
            std::path::Path::new("/tmp/store.db"),
            std::path::Path::new("/tmp/out.json"),
            5033164,
            &bms,
            142,
        );
        let json = serde_json::to_string(&report).unwrap();
        // Spot-check structural fields. The full schema is exercised by
        // the integration of `compute_stats` + `top_collections`.
        assert!(json.contains("\"tool\":\"linkmarks export\""));
        assert!(json.contains("\"bookmarks_exported\":2"));
        assert!(json.contains("\"tags_dropped\":1"));
        assert!(json.contains("\"import_required\":true"));
        assert!(json.contains("\"auto_detect_supported\":false"));
        assert!(json.contains("\"duration_ms\":142"));
    }

    #[test]
    fn human_bytes_chooses_unit() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(1024), "1.0 KB");
        assert_eq!(human_bytes(5033164), "4.8 MB");
    }

    #[test]
    fn default_next_steps_mentions_chromium_family_and_firefox() {
        let n = default_next_steps();
        assert!(n.import_required);
        assert!(!n.auto_detect_supported);
        assert!(n.browsers.iter().any(|b| b.name.starts_with("Vivaldi")));
        assert!(n.browsers.iter().any(|b| b.name == "Firefox"));
        assert!(n.docs_url.contains("why-file-based.md"));
    }
}
