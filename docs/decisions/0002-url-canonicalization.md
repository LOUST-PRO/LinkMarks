# ADR 0002 — URL Canonicalization

- **Status**: accepted
- **Date**: 2026-07-28
- **Supersedes**: ADR-0001 (v1.0.0 inline doc-comment; this ADR is the canonical record)

## Context and Problem Statement

LinkMarks deduplicates bookmarks by canonical URL. Two imports of the same page — one from Chrome with `utm_source=newsletter`, one from Firefox with `fbclid=xyz` — must collapse to a single entry. The dedupe key is the canonical URL string. A wrong rule silently loses data (two distinct pages merged) or inflates storage (two copies of the same page kept).

The tracking-parameter landscape is large, evolving, and politically charged: vendors add new click IDs every few years, and blocklists rot. We need a deterministic, documented, test-pinned ruleset that survives audits and operator review.

### Forces at play

- **Correctness > convenience**: silent dedupe mistakes are worse than no dedupe at all.
- **Predictability > optimization**: the canonical URL must round-trip; users expect `linkmarks list` to show the URL they would click.
- **Extensibility without surprise**: operators want to override rules per domain (e.g. preserve Amazon's `tag` for affiliate integrity) without rewriting core code.
- **Auditability**: every blocklist entry must trace to a source (vendor documentation, well-known list, peer review).

## Decision Drivers

1. Determinism: same input → same output across runs, platforms, processes.
2. Auditable blocklist: each entry cited or sourced.
3. Per-domain override without code changes (config file).
4. Backwards-compatible: rules from v1.0.0 stay valid in v1.x.
5. Minimal surface area: no regex engine, no plugin runtime; just a HashMap of `host → preserve_params`.

## Considered Options

### Option A — regex-based rules per domain

```text
[canonical.rules]
"amazon\\.com" = "tag=*|ref_*"
```

**Rejected**. Regex is hostile to audit. Reviewers must mentally parse; a typo silently drops all `tag` params. Hard to test.

### Option B — external blocklist fetched at runtime

**Rejected**. Breaks local-first. Adds network dependency at import time. Blocklist can be poisoned. Loses offline guarantee that LinkMarks is built on.

### Option C — blocklist in code + per-domain allowlist in config.toml (CHOSEN)

The blocklist lives in `canonical.rs` as a `const &[&str]`, with prefix-match rules (`utm_*`, `mc_*`) and exact-match rules. The per-domain allowlist lives in `config.toml.example` as a HashMap `host → preserve_params`. Operators edit config, never code.

## Decision

We adopt **Option C**. The implementation lives in:

- `crates/linkmarks-core/src/canonical.rs` — ruleset + `canonicalize()` + `is_tracking()` + `canonicalize_with()`.
- `crates/linkmarks-core/src/canonical_config.rs` — `CanonicalConfig`, `DomainRules`, `ALWAYS_FUNCTIONAL`.
- `config.toml.example` — operator-facing per-domain overrides (YouTube, Vimeo, GitHub, Twitter/X, Amazon).

### The canonicalization ruleset (in order)

1. **Parse** with the `url` crate. Empty input is rejected.
2. **Lowercase scheme**. `HTTPS://` → `https://`.
3. **Lowercase host** (ASCII). IDN hosts already punycoded by `url::Host::Domain`. IPv6 literals lowercased for hex-digit safety.
4. **Strip default port**: `:80` for http, `:443` for https, `:21` for ftp.
5. **Drop fragment** (`#...`). Bookmarks don't carry UI state.
6. **Filter and sort query parameters** by lowercase key.
7. **Drop tracking parameters** unless `config.is_preserved(host, param)` says otherwise.
8. **Strip trailing slash** from non-root paths.
9. **Re-serialize** and **round-trip parse** as a sanity check.

### The tracking-parameter blocklist

Located at `crates/linkmarks-core/src/canonical.rs:TRACKING_PARAMS`. Sources:

| Param | Source | Introduced | Notes |
|---|---|---|---|
| `utm_*` | Google Analytics | ~2005 | Most common. **Prefix match**. |
| `mc_*` | Mailchimp | — | **Prefix match** (`mc_eid`, `mc_cid`, etc.). |
| `fbclid` | Facebook | 2018 | First-party click ID. |
| `gclid` | Google Ads | ~2013 | |
| `gbraid` | Google Ads iOS | 2021 | |
| `wbraid` | Google Ads web | 2021 | |
| `msclkid` | Microsoft Bing Ads | — | |
| `dclid` | DoubleClick / GMP | — | |
| `yclid` | Yandex Direct | — | |
| `twclid` | Twitter | 2022 | Deprecated 2024, kept for historical dedupe. |
| `li_fat_id` | LinkedIn | — | First-party ad tracking. |
| `igshid` | Instagram | — | Share ID. |
| `ttclid` | TikTok | — | |
| `ref` | Generic | — | Many CMSes use this for referrer. |
| `ref_src` | Generic | — | |
| `ref_url` | Generic | — | |
| `source` | Newsletter platforms | — | Overridable per-domain (e.g. `grep.app` uses it functionally). |
| `spm` | Taobao/Alibaba | — | Internal tracking. |
| `scm` | Taobao/Alibaba | — | Internal tracking. |
| `_hsenc` | HubSpot | — | |
| `_hsmi` | HubSpot | — | |
| `mkt_tok` | Marketo | — | |

### The always-functional list

Located at `crates/linkmarks-core/src/canonical_config.rs:ALWAYS_FUNCTIONAL`:

```text
id, page, q, query, search, lang, locale, sort, order,
limit, offset, cursor, tab, filter
```

These are NEVER stripped, even if a domain-specific blocklist tries to. They denote application state, not telemetry.

### Per-domain config schema

```toml
[canonical.domains]
"youtube.com" = { preserve_params = ["t", "v", "list", "index", "si"] }
"github.com"  = { preserve_params = ["q", "tab", "type"] }
"amazon.com"  = { preserve_params = ["tag"] }
```

The operator writes the file at `~/.config/linkmarks/config.toml`. LinkMarks reads it at startup if present; absent → defaults.

## Consequences

### Positive

- **Single source of truth**: blocklist in code, overrides in config. Auditors can review both.
- **Round-trip deterministic**: same URL → same canonical across runs, platforms, processes.
- **Operator-extensible**: no Rust required to add a domain override.
- **Test-pinned**: 39 tests in `linkmarks-core` cover lowercase scheme/host, default-port stripping, sort, fragment strip, tracking drop (all 21+ known params), trailing-slash rule, IDN hosts, determinism, per-domain overrides, functional param preservation.

### Negative

- **Blocklist rot**: a new `xclckid` from X Corp or `ttclid` variant from TikTok won't be stripped until the code is patched. Mitigation: quarterly review cadence (documented in CONCERNS.md §C4) and operator-driven `preserve_params` for the inverse case.
- **Per-domain config is opt-in**: an operator unaware of the example config will get safe defaults, but they will lose functional params on a few sites (YouTube `t`, Amazon `tag`). Mitigation: `config.toml.example` ships in the repo root and is referenced in `linkmarks init`.
- **No way to BLOCK a functional param per domain** without code change. This is intentional — the asymmetry is "strip by default, preserve on demand", which favors data minimization.

### Neutral

- The ruleset is documented in code (doc-comment on `canonicalize`) and in this ADR. The two must stay in sync; CI does not enforce this. Mitigation: when editing `TRACKING_PARAMS`, also update this ADR table.

## Implementation Notes

- `canonicalize()` is the public entry point with safe defaults. `canonicalize_with(&CanonicalConfig)` is the variant that respects custom rules. CLI/TUI use `canonicalize()`; tests use `canonicalize_with()`.
- The config file is parsed in Fase 2 (TUI). Fase 1 only ships the data structures and `default_rules()`.
- Test corpus lives in `tests/fixtures/chromium/` and `tests/fixtures/firefox/`. Each fixture is anonymized (no PII, no real URLs) per CONCERNS.md §C5.

## Validation Evidence

- `cargo test --package linkmarks-core --lib` → 39/39 passed.
- `cargo build --release` → OK (0 warnings).
- `cargo clippy --all-targets -- -D warnings` → 0 warnings.
- Round-trip fixtures in `tests/fixtures/chromium/` (10+ files) pass `import → export → re-import → list` byte-identical.

## Related

- ADR-0001 (v1.0.0 inline doc-comment — now superseded by this ADR).
- CONCERNS.md §C4 — blocklist review cadence.
- CONCERNS.md §C5 — PII sanitization rules.
- `config.toml.example` — operator-facing per-domain config template.