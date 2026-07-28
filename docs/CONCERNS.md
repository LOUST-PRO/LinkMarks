# LinkMarks — Open Concerns

**Status**: Documentation staging. Each concern below is awaiting
operator input or evidence collection.
**Date**: 2026-07-28

Format: each concern has a **status** (`open` / `pending-approval` /
`mitigated` / `resolved`) and a **decision needed from** line.

---

## C1 — AGPLv3 + Commercial dual licensing

**Status**: pending-approval (validated pattern, not yet applied to LinkMarks)

**Context**: `lzt-pr-auto-tagger` (verified 2026-07-28 at
`~/Proyectos/OSS/LOUST-PRO/lzt-pr-auto-tagger/`) ships with:
- `LICENSE` — AGPLv3
- `LICENSE-COMMERCIAL.md` — commercial terms for entities that can't
  comply with AGPLv3

This is the Appwrite / HashiCorp / Sentry dual-license pattern. It
works when:
- The OSS code is genuinely useful under AGPLv3 (network use clause
  forces SaaS users to either open-source their modifications or pay).
- A commercial license exists for non-network / on-prem / embedded use.

**For LinkMarks**: same pattern is plausible because:
- The server relay is network-exposed (AGPLv3 covers this).
- Some enterprise users (banks, gov) cannot run AGPLv3 code at all
  due to procurement policy. Commercial license lets them pay to
  relicense.

**Decision needed from**: operator — confirm AGPLv3 + Commercial dual
is the chosen license. If yes, draft `LICENSE-COMMERCIAL.md` template
(mirror `lzt-pr-auto-tagger` wording or simpler).

**Risk if rejected**: a single AGPLv3 license is fine but loses the
dual-license monetization lever. A non-AGPL license (MIT/Apache) is
fine but loses the SaaS-share clause. Either is defensible; the
choice is strategic.

---

## C2 — Linkwarden migration path

**Status**: open

**Context**: Linkwarden is the closest comparable OSS bookmark manager.
Users coming from Linkwarden are a likely early-adopter segment. The
operator decided NOT to fork Linkwarden (re-use the spec, write Rust
from scratch). But migration of existing Linkwarden data is still a
legitimate need.

**3 options**:

### Option A — Linkwarden API bridge

- Requires the user to have a running Linkwarden instance + API token.
- Reads Linkwarden's REST API, maps to `linkmarks_core::Bookmark`,
  writes to local store.
- Pros: covers live data, including collections and tags as Linkwarden
  stores them.
- Cons: requires Linkwarden running; doesn't help users with a static
  export only.

### Option B — Linkwarden JSON/HTML export bridge

- Linkwarden can export to a JSON file or Netscape HTML.
- Read the export, map to LinkMarks model.
- Pros: works without a running Linkwarden.
- Cons: may miss fields Linkwarden doesn't export (read positions,
  highlights if any).

### Option C — Dedicated migrator binary

- A separate `linkmarks-migrate-linkwarden` binary that reads
  Linkwarden's own SQLite directly (Linkwarden uses SQLite for its
  store, similar to our own choice).
- Pros: full fidelity, including fields the API/export omit.
- Cons: depends on Linkwarden's internal schema; if they change it,
  the migrator breaks.

**Decision needed from**: operator — which option(s) ship in Fase 2?
Recommendation: ship **B only** in v0.2.0 (lowest cost, works for most
users). Add **A** if there's demand. **C** is overkill until users
report missing data.

**Tracking**: this concern defers until Fase 2 planning starts.

---

## C3 — Hosted SaaS pricing (hypothesis)

**Status**: hypothesis only, NOT a commitment

**Context**: A SaaS persona exists (SPEC.md §3). Pricing is not set.
Mentioning numbers here is to anchor the conversation, not to commit.

**Hypothetical tiers** (for discussion only):

| Tier | Price | Quota | Notes |
|---|---|---|---|
| Free | $0 | 1000 bookmarks, 1 device | No credit card, AGPLv3 spirit |
| Pro | $5/mo | 25k bookmarks, 3 devices | Self-serve, Stripe |
| Team | $15/user/mo | 100k bookmarks, 10 users | Shared collections |
| Self-host | $0 + commercial license quote | Unlimited | For enterprises that can't run AGPL |

**Decision needed from**: operator — when does pricing enter the
conversation? Recommendation: not before Fase 3 ships. Pricing is a
Fase 4+ concern; locking numbers too early locks feature priority
wrong.

**Risk**: pricing too low → can't fund server infra. Pricing too high
→ self-host users leave. The dual-license model is the hedge — SaaS
revenue is upside, not the only path.

**Anti-feature reminder**: SaaS is optional. The OSS project is the
product. SaaS revenue funds the OSS work, not the other way around.

---

## C4 — Positioning vs Linkwarden

**Status**: open (no public statements yet)

**Context**: Linkwarden is established. LinkMarks is a new entrant.
Three positioning options:

### Option A — Complement, not competitor

- "LinkMarks is the CLI-first local-first alternative for power users
  who don't want a web UI as their primary surface."
- Audience: terminal-comfortable users, tinkerers, privacy-conscious.
- Risk: small market.

### Option B — Direct competitor

- "LinkMarks does what Linkwarden does, but with a Rust core, AGPLv3
  + commercial, and explicit anti-features (no telemetry, no
  AI-without-cost-control)."
- Audience: anyone comparing the two.
- Risk: hostile framing makes the OSS community pick sides.

### Option C — Niche differentiator

- "LinkMarks is for users with multi-source bookmark mess (Chromium +
  Firefox + Pinboard + Raindrop) who want one canonical store."
- Audience: specifically people with the dedupe problem.
- Risk: niche-y; may not scale.

**Decision needed from**: operator — which positioning for the
README, blog posts, and any later OSS announcement? Recommendation:
**C**, with **A** as the secondary note. Avoid **B** until LinkMarks
has shipped at least one release with real users.

**Tracking**: this is a Fase 4+ decision (after the project has a
public face).

---

## C5 — Technical concerns

### C5.1 — URL canonicalization edge cases

**Concern**: URL canonicalization is the dedupe key. Wrong canonical
= wrong dedupe = silent data loss.

**Known tricky cases**:
- IDN hosts (`xn--` vs Unicode) — must canonicalize one way, not mix.
- Default ports (`http://example.com:80/` vs `http://example.com/`).
- Trailing slash on root (`http://example.com` vs `http://example.com/`).
- Query param order — sort, or preserve?
- Fragment — strip or preserve?
- Tracking params — which list to use? `utm_*` is standard but new
  ones appear constantly.

**Mitigation**:
- Use the `url` crate as the parser (battle-tested).
- Document the canonicalization rules in `docs/decisions/01-canonical-url.md`
  with examples.
- Pin a "tracking param blocklist" with a date; revisit quarterly.
- Add fixture tests covering 50+ real-world URLs to lock behavior.

**Status**: pending-approval (rules to be drafted before Fase 1 ships).

### C5.2 — Privacy of external APIs

**Concern**: Pinboard and Linkwarden bridges need API tokens. Tokens
in local config = leak risk.

**Mitigation**:
- Tokens read from env vars or `~/.config/linkmarks/credentials`
  with `0600` mode.
- Tokens never logged, never serialized into the local store.
- Token value masked in `linkmarks config show` (show first 4 chars
  only).
- No token ever sent to a LinkMarks-controlled server (we don't have
  one).

**Status**: resolved-by-default (design rule, will land in code).

### C5.3 — Plugin security (Fase 5)

**Concern**: third-party plugins run with the user's credentials
(Pinboard API token, Linkwarden token, etc.). A malicious plugin
exfiltrates.

**Mitigation**:
- Plugins run in subprocess (process isolation, not just memory
  isolation).
- Capability declaration in manifest (network access, file access)
  must be granted by user at install time.
- Plugin binaries signed by a key the operator publishes (similar to
  sigstore / npm publish).
- Plugin registry metadata is auditable (open-source, anyone can
  mirror).

**Status**: open (Fase 5 concern, design work starts after Fase 4
ships).

### C5.4 — CRDT document format (Fase 3)

**Concern**: if we lock `yrs` and later want to migrate to
`automerge-rs` (or a future CRDT), the document format is opaque.
Migration = data loss.

**Mitigation**:
- Spike (Fase 3) compares not just runtime perf but also document
  portability.
- Server stores CRDT blobs as opaque bytes; clients handle format
  negotiation.
- Periodic (quarterly) export to Netscape HTML as a fallback safety
  net for users.

**Status**: open (Fase 3 concern).

### C5.5 — Browser profile locks

**Concern**: Firefox import reads `places.sqlite`. If Firefox is
running, the SQLite file is locked (WAL mode + lock file). Reading
fails.

**Mitigation**:
- Bridge operates read-only; we never write to the profile.
- Document: user must close Firefox or copy the profile to a temp
  dir.
- Provide `linkmarks import firefox --copy-profile` that copies first
  then reads (slower but safe).

**Status**: resolved-by-default (mitigation built into bridge design).

### C5.6 — Test fixtures

**Concern**: fixture corpus (real Chromium / Firefox exports) may
contain PII (URLs with embedded session tokens, personal bookmarks).

**Mitigation**:
- All fixtures anonymized: titles replaced with `Title N`, URLs
  rewritten to example.com subdomains, tags randomized.
- Pre-commit hook greps fixtures for known-PII patterns.
- Originals stored encrypted in `tests/fixtures/_private/` (gitignored).

**Status**: pending-approval (rule to be enforced before Fase 1
fixtures are committed).

### C5.7 — Memory limits on large stores

**Concern**: a user with 50k bookmarks. CLI `list` must not OOM.

**Mitigation**:
- `list` uses streaming iterators; never loads all rows into memory.
- TUI uses pagination with a configurable page size (default 100).
- Server caps each WebSocket frame at 1 MiB; clients paginate.

**Status**: resolved-by-default (architectural constraint, not a
deferrable decision).

### C5.8 — Server auth (Fase 3)

**Concern**: API key auth is simple but has no rotation story, no
multi-device per user cleanly.

**Mitigation for v1**:
- One API key per client (laptop, phone, GUI each get their own).
- Key rotation: revoke old, issue new, no overlap window.
- Multi-device = multiple API keys under one user account.

**Status**: open (Fase 3 design concern; revisit when Fase 3 starts).

---

## C6 — Risks not yet listed

These are meta-risks that don't fit above:

1. **Scope creep into "read-later" territory.** SPEC.md §anti-features
   explicitly forbids this, but pressure will come. The CONCERNS.md
   list exists so we can point to it and decline cleanly.
2. **Burnout from being a one-operator project.** LinkMarks is a
   substantial codebase. Without co-maintainers, bus-factor = 1.
   Mitigation: write all decisions as ADRs so a future maintainer can
   onboard.
3. **Marketing cost for an OSS project nobody knows.** Even with a
   good product, distribution is hard. Mitigation: lean into the
   power-user niche (they find the project via search, not ads).
4. **CRDT lock-in.** If `yrs` becomes unmaintained, our sync layer
   breaks. Mitigation: keep CRDT code isolated behind
   `SyncAdapter` trait; swap implementations without touching clients.

---

## How to update this file

When a concern is resolved:
1. Move it from `open` / `pending-approval` to `resolved` or
   `mitigated`.
2. Link to the ADR or commit that resolved it.
3. Do NOT delete the concern — historical record matters.

When a new concern appears:
1. Add it to the appropriate section (C1-C6).
2. Default status: `open`.
3. Cite the source (SPEC.md reference, ROADMAP.md reference, or
   external trigger).
