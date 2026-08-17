//! Deterministic 10k-bookmark synthetic fixture.
//!
//! Shape mirrors `linkmarks-core::Bookmark` (ULID id, original_url,
//! canonical_url, optional title/description/collection/content_type,
//! tags vec, created_at + updated_at, `SourceKind` enum, archived flag)
//! — but the field set is mirrored locally so the bench crate stays
//! compilable independently of `linkmarks-core`'s release cycle.
//!
//! The PRNG seed is fixed to 42 (`ChaCha8Rng`) so encode-size
//! measurements byte-exactly reproduce across runs.

use chrono::{DateTime, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// Mirrors `linkmarks-core::SourceKind`. Local copy avoids a path
/// dep on `linkmarks-core` so the POC compiles in isolation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Chromium,
    Firefox,
    Netscape,
    Pinboard,
    Linkwarden,
    Manual,
}

/// Synthetic bookmark. Same field shape as `linkmarks-core::Bookmark` —
/// if the production model grows a field, update this struct in the
/// same commit so encode benchmarks stay representative.
#[derive(Clone, Debug, PartialEq)]
pub struct BenchBookmark {
    pub id: String,
    pub original_url: String,
    pub canonical_url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub collection: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub source: SourceKind,
    pub content_type: Option<String>,
    pub archived: bool,
}

// Pool sizes picked to give the workload the same de-/serialization
// breadth as a real-world 10k-bookmark library. Domains are real-shape
// (no fabricated TLDs); tags / collections / content_types are the
// canonical buckets linkmarks canonical-URL classification buckets into.

const DOMAINS: &[&str] = &[
    "github.com",
    "stackoverflow.com",
    "en.wikipedia.org",
    "news.ycombinator.com",
    "reddit.com",
    "x.com",
    "youtube.com",
    "youtu.be",
    "medium.com",
    "arxiv.org",
    "acm.org",
    "ieee.org",
    "nytimes.com",
    "theguardian.com",
    "wired.com",
    "arstechnica.com",
    "lobste.rs",
    "dev.to",
    "hashnode.com",
    "openai.com",
    "anthropic.com",
    "loust.pro",
    "linkedin.com",
    "instagram.com",
    "tiktok.com",
    "twitch.tv",
    "amazon.com",
    "archive.org",
    "nasa.gov",
    "rust-lang.org",
    "docs.rs",
    "crates.io",
    "gitlab.com",
    "bbc.co.uk",
    "cnn.com",
    "reuters.com",
    "apnews.com",
    "bloomberg.com",
    "wsj.com",
    "ft.com",
    "economist.com",
    "nature.com",
];

const TAG_POOL: &[&str] = &[
    "rust",
    "python",
    "golang",
    "javascript",
    "typescript",
    "web",
    "devops",
    "kubernetes",
    "docker",
    "aws",
    "linux",
    "macos",
    "ai",
    "ml",
    "llm",
    "transformers",
    "rag",
    "vector-db",
    "embeddings",
    "security",
    "networking",
    "tcp",
    "udp",
    "http",
    "https",
    "rest-api",
    "graphql",
    "wasm",
    "compilers",
    "regex",
    "database",
    "sql",
    "nosql",
    "sqlite",
    "postgres",
    "redis",
    "tutorial",
    "documentation",
    "reference",
    "design",
    "typography",
    "ux",
    "ui",
    "research",
    "paper",
    "book",
    "podcast",
    "video",
    "talk",
];

const COLLECTION_POOL: &[&str] = &[
    "inbox",
    "to-read",
    "reference",
    "shopping",
    "work",
    "personal",
    "research",
    "tools",
    "learning",
    "rust",
    "ai",
    "security",
    "design",
    "ops",
    "archive",
];

const CONTENT_TYPES: &[&str] = &[
    "text/html",
    "application/pdf",
    "application/json",
    "image/png",
    "image/jpeg",
    "video/mp4",
    "text/plain",
];

/// Build a deterministic fixture of `n` bookmarks.
///
/// `seed = 42` is fixed (see `standard_fixture` and tests) so the
/// generated workload is byte-exactly the same on every run —
/// encode-size measurements cannot drift between runs.
pub fn generate_fixture(n: usize) -> Vec<BenchBookmark> {
    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let now = Utc::now();
    let five_years_ago = now - chrono::Duration::days(365 * 5);

    (0..n)
        .map(|i| {
            let domain = DOMAINS[rng.gen_range(0..DOMAINS.len())];
            let path = format!("/article/{i}");
            let original_url = format!("https://{domain}{path}");
            let canonical_url = format!("https://{domain}{path}");

            let title = format!("Article #{i} on {domain}");
            let description = format!("Description for bookmark {i}");

            // 0-5 tags, drawn without replacement from TAG_POOL.
            let tag_count = rng.gen_range(0..=5);
            let mut tags: Vec<String> = Vec::with_capacity(tag_count);
            let mut attempts = 0;
            while tags.len() < tag_count && attempts < 16 {
                let tag = TAG_POOL[rng.gen_range(0..TAG_POOL.len())].to_string();
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
                attempts += 1;
            }

            // ~70% of real bookmarks belong to a collection.
            let collection = if rng.gen_bool(0.7) {
                Some(COLLECTION_POOL[rng.gen_range(0..COLLECTION_POOL.len())].to_string())
            } else {
                None
            };

            // created_at spread over 5y; updated_at in [created_at, now].
            let created_secs = rng.gen_range(five_years_ago.timestamp()..=now.timestamp());
            let created_at = DateTime::<Utc>::from_timestamp(created_secs, 0)
                .expect("valid timestamp in [now-5y, now]");
            let max_delta_secs = (now.timestamp() - created_at.timestamp()).max(0);
            let updated_at = if max_delta_secs > 0 {
                let delta = rng.gen_range(0..=max_delta_secs);
                created_at + chrono::Duration::seconds(delta)
            } else {
                created_at
            };

            let source = match rng.gen_range(0..6) {
                0 => SourceKind::Chromium,
                1 => SourceKind::Firefox,
                2 => SourceKind::Netscape,
                3 => SourceKind::Pinboard,
                4 => SourceKind::Linkwarden,
                _ => SourceKind::Manual,
            };

            let content_type =
                CONTENT_TYPES[rng.gen_range(0..CONTENT_TYPES.len())].to_string();

            // ~5% archived (the "I archive instead of delete" crowd).
            let archived = rng.gen_bool(0.05);

            BenchBookmark {
                // Deterministic ULID: timestamp from `created_at`, random
                // bytes drawn from the same seeded ChaCha8Rng used for the
                // rest of the field generation. Avoids `Ulid::generate()`
                // which would hit `rand::thread_rng()` and break
                // determinism between successive `generate_fixture(n)` calls.
                // ULID 3.x `from_parts` takes a `u128` random; pack the
                // 10 bytes (LE) into it.
                id: {
                    let ulid_bytes: [u8; 16] = std::array::from_fn(|_| rng.gen());
                    let random_u128 = u128::from_le_bytes(ulid_bytes);
                    ulid::Ulid::from_parts(
                        created_at.timestamp_millis() as u64,
                        random_u128,
                    )
                    .to_string()
                },
                original_url,
                canonical_url,
                title: Some(title),
                description: Some(description),
                tags,
                collection,
                created_at,
                updated_at,
                source,
                content_type: Some(content_type),
                archived,
            }
        })
        .collect()
}

/// The standard 10k-bookmark workload used by both benches.
pub fn standard_fixture() -> Vec<BenchBookmark> {
    generate_fixture(10_000)
}
