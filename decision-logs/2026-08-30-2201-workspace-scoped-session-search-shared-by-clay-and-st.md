---
date: 2026-08-30 22:01
status: approved
decision_about: "Workspace-scoped session search shared by clay and st"
proposed_by: both
explicitly_approved_by_user: true
---

# Decision: One workspace-scoped SQLite session search for clay and `st`

## Decision

Clay uses one SQLite session/branch/checkpoint store for both the base
coding agent and `st`, keyed by a stable workspace identity. Clay owns a
workspace-scoped SQLite FTS index over user and agent messages, branch
summaries, plan/task metadata, and observational-memory reflections.
Raw tool output is not indexed by default. One `session.search` RPC and
session picker/command surface search only the current workspace and
return the session, branch, matching tree entry/snippet, timestamp, and
status. Selecting a result resumes or opens that session at the matching
tree entry. Results are transcript data, not automatic agent context:
the user must explicitly open or attach them. `st` writes workflow type,
run, phase, task, checkpoint, and status annotations into that same
index; it does not create a second session store.

## Context

The existing roadmap had `prism-session-store-sqlite` persistence for
sessions, branches, and checkpoints (Phase 1), then session list/resume
and pi-style tree navigation (Phase 2). It did not define search across
all sessions belonging to a workspace. The current daemon's
`sessionResume` is by `sessionId`, confirming that a workspace-scoped
search contract is a new capability, not implicit behavior.

## Approval

- Proposed by: both (agent recommendation, user adoption)
- Approved by user: Yes
- Approval evidence: after receiving the recommended shared,
  workspace-scoped FTS design, user said: "Let's go with your
  recommendation. Add this to roadmap and decision log" (2026-08-30).

## Alternatives Considered

1. **Separate `st` session store** — rejected: duplicates branches,
   checkpoints, UI, and search mechanics; `st` is an extension of the
   base coding agent and needs only extra annotations.
2. **Global cross-workspace search** — rejected for v1: weakens workspace
   isolation and raises relevance/privacy problems. Search current
   workspace only.
3. **Index all raw tool output** — rejected by default: tool output is
   noisy, can be large, and may contain untrusted/sensitive material.
   Search durable human/agent text, summaries, plans/tasks, and OM
   reflections instead.
4. **Automatically inject search hits into current agent context** —
   rejected: surprises the user and may pull untrusted/irrelevant prior
   transcript into a run. Opening/attaching is explicit.

## Rationale and Evidence

- `prism-session-store-sqlite` is already the roadmap's durable store
  for session/branch/checkpoint persistence; SQLite FTS is the smallest
  local search extension, requiring no service or additional database.
- pi-style session trees give every indexed match an exact branch/entry
  location, so a search hit can restore the relevant conversation
  position rather than merely show text.
- `st` workflow labels make autonomous work discoverable in the same
  search surface without duplicating storage.
- The base and `st` share one workspace and session UI; a single index
  preserves that product model.

## References

- `roadmap.md` — Prism Adoption Map (`prism-session-store-sqlite` row),
  Phase 1 (workspace-scoped session index), Phase 2 (session search +
  isolation exit-gate drill), Phase 5 (`st` annotations), resolved item 9.
- `clay-agent/src/host.ts:L339-L343` — current `sessionResume` accepts a
  `sessionId`; no workspace search contract.
- pi `docs/session-format.md` — session tree entry identity and branch
  navigation model.
- Decision log `2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md` — linked session/document/git checkpoint model.

## Consequences

- Positive: users can find and resume any relevant clay or `st` session
  in the current workspace; no duplicate storage; search respects the
  context trust boundary.
- Risks: transcript indexing increases SQLite data and requires schema
  migrations; raw-output exclusion may omit a useful technical detail.
- Revisit if: users need intentional cross-workspace search, raw-output
  indexing, semantic/vector retrieval, or shared cross-session OM scopes.
