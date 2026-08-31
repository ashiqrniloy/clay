---
date: 2026-08-30 21:53
status: approved
decision_about: "st package identity and distribution channel"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: `st` package named `@arnilo/st`, distributed via npm (pi model)

## Decision

The autonomous agent package is named `st`, published as `@arnilo/st` on
npm. Distribution follows the pi package model fully: npm and GitHub
releases as the two initial sources. Clay itself is `@arnilo/clay` on npm;
Homebrew distribution is skipped for now (post-roadmap).

## Context

Clay's roadmap (Phase 5) needs a concrete package identity for the
user's autonomous coding agent before implementation planning. Prism's
package graph and Clay's `clay install` flow (Phase 3) require the npm
scope and install mechanics to be fixed.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: user stated the package name would be `st`, placed
  under `@arnilo/st`, distributed via npm, following the pi model fully,
  with npm and GitHub releases as initial sources (2026-08-29). Approved
  for logging: "Go ahead and log the decisions as decided already"
  (2026-08-30).

## Alternatives Considered

1. **Other scopes/names** — rejected; `@arnilo` is the existing scope
   (`@arnilo/prism`), `st` is the user's chosen name.
2. **GitHub-only distribution** — rejected; npm is required for
   `clay install npm:@arnilo/st` parity with pi.
3. **Homebrew now** — deferred explicitly; npm covers Phase 3.

## Rationale and Evidence

- Prism already publishes under `@arnilo` (prism, prism-workflows,
  prism-supervisor, …), so `@arnilo/st` is consistent.
- The pi package model (npm + GitHub releases, `install npm:` /
   `install github:` forms) is already the template for Clay's
  `clay install`/`clay remove`/`clay update` design in roadmap Phase 3.
- `clay install` appends the package to `loadPackage` in `init.js`
  (user-specified mechanism).

## References

- `roadmap.md` — Phase 3 (Package Installation, Update, and Clay
  Distribution), Phase 5 (`st` Package — Orchestration Core).
- Decision log 2026-08-21-1758 — native Prism host, no ACP; `st` is the
  CLI-parity coding agent built from primitives.
- Decision log 2026-05-08-1958 — Clay JS API naming and package
  distribution precedent.

## Consequences

- Positive: single scope, pi-model install parity, `st` inherits the
  clay coding agent via package extension.
- Risks: `st` name is short and generic on npm; the scoped
  `@arnilo/st` avoids collisions.
- Revisit if: distribution needs channels beyond npm/GitHub releases,
  or Homebrew is pulled forward from post-roadmap.