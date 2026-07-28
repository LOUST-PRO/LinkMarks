//! Output formatting helpers (table / json / yaml).

use anyhow::Result;
use linkmarks_core::Bookmark;
use serde::Serialize;

/// A row in the deterministic table view.
///
/// Column order is fixed: `id | canonical_url | title | tags | collection | updated_at`.
#[derive(Debug, Serialize)]
pub struct TableRow<'a> {
    pub id: &'a str,
    pub canonical_url: &'a str,
    pub title: &'a str,
    pub tags: String,
    pub collection: String,
    pub updated_at: String,
}

impl<'a> From<&'a Bookmark> for TableRow<'a> {
    fn from(b: &'a Bookmark) -> Self {
        Self {
            id: &b.id.0,
            canonical_url: &b.canonical_url,
            title: &b.title,
            tags: b.tags.join(","),
            collection: b.collection.clone().unwrap_or_default(),
            updated_at: b.updated_at.to_rfc3339(),
        }
    }
}

/// Sort + serialize bookmarks into the requested format.
pub fn render(bookmarks: &[Bookmark], format: super::Format) -> Result<String> {
    // Deterministic ordering: canonical URL asc, then id asc.
    let mut sorted: Vec<&Bookmark> = bookmarks.iter().collect();
    sorted.sort_by(|a, b| {
        a.canonical_url
            .cmp(&b.canonical_url)
            .then_with(|| a.id.0.cmp(&b.id.0))
    });

    match format {
        super::Format::Table => render_table(&sorted),
        super::Format::Json => render_json(&sorted),
        super::Format::Yaml => render_yaml(&sorted),
    }
}

fn render_table(sorted: &[&Bookmark]) -> Result<String> {
    let mut out = String::new();
    out.push_str("id\tcanonical_url\ttitle\ttags\tcollection\tupdated_at\n");
    for b in sorted {
        let row = TableRow::from(*b);
        out.push_str(&format!(
            "{id}\t{canonical}\t{title}\t{tags}\t{collection}\t{updated}\n",
            id = row.id,
            canonical = row.canonical_url,
            title = row.title,
            tags = row.tags,
            collection = row.collection,
            updated = row.updated_at,
        ));
    }
    Ok(out)
}

fn render_json(sorted: &[&Bookmark]) -> Result<String> {
    // NDJSON: one bookmark per line, already sorted by the caller.
    let mut out = String::new();
    for b in sorted {
        out.push_str(&serde_json::to_string(b)?);
        out.push('\n');
    }
    Ok(out)
}

fn render_yaml(sorted: &[&Bookmark]) -> Result<String> {
    let value = serde_yaml::to_string(sorted)?;
    Ok(value)
}
