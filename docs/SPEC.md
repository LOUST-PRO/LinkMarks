# LinkMarks — Product Spec

**Status**: Documentation staging. No code, no repo.
**Date**: 2026-07-28
**Target**: `LOUST-PRO/LinkMarks` (private)

## One-line pitch

A local-first, AGPL-licensed bookmark manager that imports what you
already have, dedupes deterministically, and stays useful offline.
Server is optional relay, not authority.

## Personas

### 1. Self-host power user

**Profile**: Runs services on their own VPS or homelab. Comfortable with
CLI, reads source before trusting, prefers AGPL over SaaS lock-in. Has
5000+ bookmarks scattered across Chromium profile, Firefox profile,
and a Pinboard account they never finished migrating away from.

**Wants**:
- One canonical store that survives browser reinstalls.
- Deterministic dedupe — same input, same output, every run.
- Diffs visible before they accept a merge.
- Server they can read, audit, and self-host.

**Not wants**:
- "AI-powered" anything that costs money to dedupe.
- Vendor accounts.
- Docker-only deploys.
- Silent telemetry.

### 2. Casual user

**Profile**: Has a few hundred bookmarks in Chrome. Tried Raindrop and
Pocket, churned because of feature bloat. Wants "import → organize →
sometimes search → occasional export". Does not want a server.

**Wants**:
- `linkmarks import ~/Downloads/Bookmarks` and it's done.
- A GUI they can open once a week to clean up.
- Read-only sync between laptop and phone without paying anyone.

**Not wants**:
- Another account.
- Setup wizards.
- Telemetry that phones home.

### 3. SaaS customer (hypothetical)

**Profile**: Small team that wants shared bookmarks without spinning
infra. Comfortable paying $5-15/month for a hosted relay. Would NOT
self-host even if free.

**Wants**:
- Email signup, done.
- Shared collections with teammates.
- A web UI that doesn't require a desktop app.

**Not wants**:
- Vendor lock-in (hence the AGPL — if we vanish, they can self-host
  the same binary they were using).

**Note**: pricing, tiering, and SLA for this persona are NOT decided.
See CONCERNS.md. The SaaS persona is a hypothesis, not a commitment.

## v1 must-have features (exact 5)

These are the only features the v1 release ships with. Anything else
is explicitly out of scope and gets tracked in ROADMAP.md or
CONCERNS.md.

### Feature 1 — Import Chromium bookmarks from JSON

- **Input**: Chromium `Bookmarks` file (path passed via flag).
  Standard location: `~/.config/google-chrome/Default/Bookmarks`.
- **Behavior**: parse the JSON, walk `roots.*.children`, normalize each
  bookmark into `linkmarks_core::Bookmark`, write to local store.
- **Output**: NDJSON of imported bookmarks + count summary.
- **Determinism**: same input → same output bytes. No timestamps from
  wall clock in the imported set; `imported_at` is the wall clock at
  import time and is reported, not hashed.
- **Exit codes**: 0 success, 1 partial (some bookmarks failed to parse,
  rest imported), 2 file not found / not readable.

### Feature 2 — `linkmarks list --source=chrome`

- **Output**: deterministic table. Column order fixed:
  `id | canonical_url | title | tags | collection | updated_at`.
- **Sort**: by canonical URL ascending, then by id ascending. Same
  order every run.
- **Formats**: `table` (default), `json` (NDJSON), `yaml`. `csv` is
  NOT v1.
- **Filtering**: `--source=chrome` only for v1. Multi-source filter is
  Fase 2.
- **Empty state**: print header + zero rows, no error.

### Feature 3 — Normalized bookmark model

Every imported bookmark is stored with:
- `original_url` (verbatim from source — never rewritten)
- `canonical_url` (normalized: lowercased host, default port stripped,
  trailing slash stripped from path, query params sorted and tracking
  params `utm_*`, `fbclid`, `gclid` dropped, fragments preserved).
- `title` (verbatim, trimmed)
- `description` (verbatim, optional)
- `tags` (sorted, lowercase, dedup'd)
- `collection` (folder path, `/`-separated, normalized)
- timestamps in UTC ISO 8601

The model is the only shape any sink receives. Bridges that produce
non-conforming shapes fail at conversion, not at storage.

### Feature 4 — Export to Netscape HTML

- **Output**: Netscape bookmark file format (the de facto interchange
  format, supported by every browser).
- **Input**: filtered subset of local store (default: all).
- **Behavior**: deterministic ordering (same as `list`). Folders become
  `<H3>` blocks; bookmarks become `<A>` with attributes.
- **Round-trip**: exporting then re-importing must produce the same
  canonical URLs (titles may differ if user edited).
- **Encoding**: UTF-8, no BOM.

### Feature 5 — Local deterministic dedupe by canonical URL with conflict report

- **Algorithm**:
  1. Group by `canonical_url`.
  2. Within each group, pick the canonical record (oldest `created_at`,
     ties broken by lowest `id`).
  3. Report all conflicts: same canonical URL but differing title /
     tags / collection.
- **Output**: human-readable conflict report + machine-readable JSON
  when `--format=json`.
- **Mode**: `--dry-run` (default) reports only; `--apply` writes
  merges. Apply requires explicit confirmation flag (not just yes/no
  prompt — a literal `--apply` token).
- **Determinism**: same store → same report → same byte order.

## v2 deferred (NOT in v1, tracked in ROADMAP.md)

- Interactive TUI browse (Fase 2)
- Firefox import (places.sqlite + jsonlz4) (Fase 2)
- Multiple source import in one pass (Fase 2)
- Dioxus GUI (Fase 4)
- CRDT sync via server (Fase 3)
- Pinboard / Linkwarden bridges (Fase 2+)
- Plugin ABI for third-party bridges (Fase 5)

## Explicitly out of scope (anti-features)

These are decisions, not gaps. Adding them later requires explicit
operator approval and a CONCERNS.md entry.

1. **Server-authoritative mode**. The server is relay-only. Clients
   own their data. No "your account is the source of truth" model.
2. **Docker-only deploy**. Server binary is single static file; systemd
   unit works. Docker is opt-in, not required.
3. **Mandatory telemetry**. No phoning home. No anonymous usage stats.
   No crash reports. Opt-in diagnostics only, off by default.
4. **Silent crawling / link health checks**. We never visit URLs to
   "check if they're alive". That costs money, leaks intent, and
   surprises the user.
5. **AI features without cost control**. No embedding generation, no
   LLM-suggested tags, no auto-summary, no semantic search. If these
   ever ship, they must:
   - be off by default,
   - declare per-action cost in the UI before running,
   - be re-runnable without re-paying,
   - never auto-run on a schedule.
6. **Closed binary build**. The release is the source. No compiled
   artifacts without matching source commit.
7. **Mobile native app**. Web responsive UI is the long-term answer;
   native apps are not planned.
8. **OCR / read-it-later**. Pocket-style article capture is not the
   product. LinkMarks is a bookmark manager, not a read-later.

## Acceptance criteria for v1

A v1 release is shippable when:

1. All 5 must-have features pass their exit codes and determinism tests.
2. `linkmarks list` on a 1000-bookmark fixture produces byte-identical
   output across 3 runs (no timestamps in stable output).
3. Round-trip: import Chromium JSON → export Netscape HTML → re-import
   Netscape HTML → `list` → first N rows identical to post-import
   Chromium listing (canonical URLs match).
4. Dedupe dry-run on a fixture with 50 conflicts produces a report
   that is byte-identical across 3 runs.
5. No network calls during any CLI command (verified by strace / ngrep
   in CI smoke).
6. No telemetry, no phoning home, no `update notifier` (verified by
   grep over the binary's strings).
