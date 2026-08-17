//! `linkmarks-bench-crdt` — CRDT benchmark suite for the project README.
//!
//! Public library surface: a deterministic fixture plus the six
//! measurement modules that back the `compare` / `compare-concurrent`
//! / `http_sync_*` binaries. The binaries run the experiments and
//! produce the per-suite `RESULTS-*.md` writeups.

pub mod fixture;
pub mod yrs_measure;
pub mod automerge_measure;
pub mod yrs_concurrent;
pub mod automerge_concurrent;

#[cfg(test)]
mod tests {
    use super::fixture::{generate_fixture, standard_fixture, SourceKind};

    /// Deterministic: same seed → byte-exact same fixture across runs.
    /// Without this, the encode-comparison measurement would be noisy.
    #[test]
    fn fixture_is_deterministic_across_runs() {
        let a = generate_fixture(100);
        let b = generate_fixture(100);
        assert_eq!(a.len(), 100);
        assert_eq!(b.len(), 100);
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.id, y.id, "id mismatch");
            assert_eq!(x.original_url, y.original_url);
            assert_eq!(x.canonical_url, y.canonical_url);
            assert_eq!(x.tags, y.tags);
            assert_eq!(x.source, y.source);
            assert_eq!(x.created_at, y.created_at);
        }
    }

    /// Real bookmark libraries are not 100% Chromium / 100% Firefox.
    /// We need all 6 SourceKinds represented for the encode bench to
    /// cover the de-/serialization breadth linkmarks actually ships.
    #[test]
    fn fixture_covers_all_source_kinds() {
        let b = generate_fixture(6_000);
        for kind in [
            SourceKind::Chromium,
            SourceKind::Firefox,
            SourceKind::Netscape,
            SourceKind::Pinboard,
            SourceKind::Linkwarden,
            SourceKind::Manual,
        ] {
            let n = b.iter().filter(|x| x.source == kind).count();
            assert!(n > 500, "SourceKind::{kind:?} underrepresented: got {n} of 6000");
        }
    }

    /// Most real bookmarks have ≥1 tag; ~5% should be tagless.
    #[test]
    fn fixture_has_realistic_tag_density() {
        let b = generate_fixture(1_000);
        let with_tags = b.iter().filter(|x| !x.tags.is_empty()).count();
        assert!(with_tags > 600, "Only {with_tags}/1000 had tags — too sparse");
        let too_many_tags = b.iter().filter(|x| x.tags.len() > 5).count();
        assert_eq!(too_many_tags, 0, "Tag pool should cap at 5");
    }

    /// Most real bookmarks belong to a collection (~70%).
    #[test]
    fn fixture_has_realistic_collection_density() {
        let b = generate_fixture(1_000);
        let with_collection = b.iter().filter(|x| x.collection.is_some()).count();
        assert!(
            (500..900).contains(&with_collection),
            "Got {with_collection}/1000 in a collection — expected 500-900 (≈70%)"
        );
    }

    /// Updated_at must be ≥ created_at (the model invariant).
    /// If the fixture violates this, the encode bench would encode
    /// an invalid bookmark shape.
    #[test]
    fn fixture_updated_at_geq_created_at() {
        let b = standard_fixture();
        for x in &b {
            assert!(
                x.updated_at >= x.created_at,
                "Bookmark {} has updated_at < created_at",
                x.id
            );
        }
    }

    /// ~5% archived — the long-tail "I never deleted it, I archived it"
    /// behaviour.
    #[test]
    fn fixture_archived_around_5_percent() {
        let b = generate_fixture(10_000);
        let archived = b.iter().filter(|x| x.archived).count();
        assert!(
            (300..700).contains(&archived),
            "Got {archived}/10000 archived — expected 300-700 (≈5%)"
        );
    }
}
