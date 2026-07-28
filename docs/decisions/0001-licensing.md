---
status: accepted
date: 2026-07-28
deciders: loust
consulted: linkmarks-architecture-plan-2026-07-28
informed: n/a
---

# 0001 — Licensing: AGPL-3.0-or-later + Commercial dual

## Context and Problem Statement

LinkMarks is a local-first bookmark manager with an optional relay
server. The relay exposes the project to a SaaS-deployment vector:
a third party could host a modified LinkMarks as a managed service
without releasing their modifications. AGPL-3.0 clause 13 ("network
use is distribution") closes that vector by requiring source
disclosure for any networked deployment.

Some prospective users — typically enterprises and government
agencies with procurement constraints — cannot run AGPLv3 code at
all. They need a path to integrate LinkMarks into proprietary
deployments without the §13 source-disclosure obligation.

How should LinkMarks be licensed?

## Considered Options

1. **MIT** — permissive, no copyleft. Loses the §13 lever entirely.
2. **Apache-2.0** — permissive + patent grant. Same as MIT for the
   §13 lever.
3. **AGPL-3.0-or-later alone** — copyleft, network clause. Closes
   SaaS loophole but locks out enterprise users that need
   proprietary deployment.
4. **AGPL-3.0-or-later + Commercial dual** — copyleft with a paid
   escape hatch. Closes the SaaS loophole for those who won't pay,
   and opens a revenue path that funds the OSS work.

## Decision Outcome

Chosen option: **4 — AGPL-3.0-or-later + Commercial dual**, "Appwrite
style".

Pattern is already validated by `lzt-pr-auto-tagger`
(`~/Proyectos/OSS/LOUST-PRO/lzt-pr-auto-tagger/`), which ships the
same dual structure. Contact point: `opensource@loust.pro`. SPDX
identifier: `AGPL-3.0-or-later`.

### Positive Consequences

- Closes the SaaS-deployment loophole via §13.
- Enterprise users with AGPL procurement blocks have a path
  forward.
- Dual revenue model funds continued OSS work.
- Pattern is already familiar to Lou's OSS community
  (lzt-pr-auto-tagger precedent).

### Negative Consequences

- Some independent SaaS operators will decline rather than pay or
  open-source their modifications. Loss is acceptable; the §13 lever
  is the goal.
- Maintainer must respond to commercial inquiries (Lou absorbs this
  load; documented in CONCERNS.md).
- Trademark: project name `LinkMarks` is not separately trademarked
  in this ADR. If trademark protection becomes needed, that is a
  separate ADR.

## Pros and Cons of the Options

### MIT / Apache-2.0

- Good, because maximum downstream adoption.
- Good, because zero legal friction for any integrator.
- Bad, because no protection against closed SaaS deployments.
- Bad, because revenue path collapses to donations or consulting.

### AGPL-3.0-or-later alone

- Good, because §13 protects against closed SaaS.
- Bad, because enterprise users with AGPL procurement blocks
  cannot integrate at all. Lost revenue + lost adoption.

### AGPL-3.0-or-later + Commercial dual (chosen)

- Good, because §13 still protects against unauthorized SaaS.
- Good, because enterprise users have a paid escape hatch.
- Good, because pattern is already proven in lzt-pr-auto-tagger.
- Bad, because a commercial inquiry pipeline adds operator load.
- Bad, because dual-licensing can confuse downstream packagers
  (Debian, Nixpkgs). Mitigation: SPDX header in every crate +
  README callout.

## Implementation Notes

- `LICENSE` — full AGPLv3 notice (mirrors lzt-pr-auto-tagger).
- `LICENSE-COMMERCIAL.md` — commercial exception wording.
- `Cargo.toml` `[workspace.package].license = "AGPL-3.0-or-later"`.
- Every crate inherits via workspace inheritance.
- Contact: `opensource@loust.pro`. No public pricing page; pricing
  is per-inquiry.

## References

- `docs/CONCERNS.md` §C1 — origin of this decision.
- `~/Proyectos/OSS/LOUST-PRO/lzt-pr-auto-tagger/LICENSE` — reference
  template.
- https://www.gnu.org/licenses/agpl-3.0.html — AGPLv3 full text.
- MADR template: https://adr.github.io/madr/
