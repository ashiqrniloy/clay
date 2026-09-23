---
date: 2026-08-30 21:58
status: approved
decision_about: "Observational memory defaults (worker model, threshold, retention)"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Observational memory — separate worker models, 80k token threshold, per-session retention

## Decision

Observational memory defaults for Clay/st: worker models (observe/
reflect/drop) are **configured separately** from the session model;
`compactAfterTokens` default is **80,000 tokens**, user-configurable;
retention is **per session** (not per workspace) to start.

## Context

Open decision 5 asked for worker models, thresholds, and retention
scope. Prism's `createObservationalMemory().attach()` supports
independent models per worker and a configurable `compactAfterTokens`;
the open questions were only the defaults. Cross-session shared scopes
remain the E5 upstream item with a working v1 workaround (parent records
delegation outcomes); per-session composition is the starting point.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "For E5: Observational memory worker model can be
  configured separately. The default threshold is 80,000 tokens but
  configurable by user. Retention per session not workspace to start
  with" (2026-08-30). Approved for logging: "Go ahead and log the
  decisions as decided already" (2026-08-30).

## Alternatives Considered

1. **Per-workspace retention** — rejected for now: cross-session OM
   scope is open upstream (E5); per-session composition plus the parent
   recording delegation outcomes covers v1.
2. **Fixed worker model = session model** — rejected: workers run
  small fast models cheaply; configurability is the point.
3. **Lower/higher fixed thresholds** — rejected: 80k is the user's
  chosen default; user-configurable makes the exact value non-critical.

## Rationale and Evidence

- `@arnilo/prism-compaction-observational-memory` exposes independent
  worker models and `compactAfterTokens` — the defaults are pure
  configuration, no code.
- Per-session retention matches the Phase 7 design: OM attaches to the
  `st` orchestrator session and each durable child session
  independently, with task-boundary compaction.
- Cross-session scopes stay post-roadmap ("if per-session composition
  proves insufficient").

## References

- `roadmap.md` — Phase 7 (memory cadence, defaults bullet), Post-
  Roadmap (cross-session shared memory scopes), resolved item 6.
- `@arnilo/prism-compaction-observational-memory` API (attach, worker
  models, compactAfterTokens).
- `docs/clay-integration-findings.md` — E5 intake item.

## Consequences

- Positive: cheap background memory work; predictable context budget;
  clean session isolation.
- Risks: per-session retention loses cross-run knowledge — mitigated
  by wiki (opt-in) for durable per-codebase knowledge.
- Revisit if: per-session proves insufficient (post-roadmap E5 path)
  or Prism ships cross-session scopes.