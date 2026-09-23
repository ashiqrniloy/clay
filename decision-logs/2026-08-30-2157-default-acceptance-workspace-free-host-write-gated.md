---
date: 2026-08-30 21:57
status: approved
decision_about: "Default acceptance policy for clay and st agents"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Default acceptance — workspace free, host write-gated, opt-in full autonomy

## Decision

Default acceptance policy for **both** the clay coding agent and `st`:
inside the operating folder (workspace) the agent can do anything and
everything with no approvals. On the host system outside the workspace
it can read everything, but any write or change requires explicit user
permission. A user-controlled **full autonomy** toggle lifts the
outside-workspace write gate for the session/run — off by default.
Egress rules (network, package installs) follow the existing sandbox /
`ExecutionPolicy` rules from `@arnilo/prism-coding-security`.

## Context

Open decision 4 asked which actions still require explicit user
approval in (fully-)autonomous runs. The user resolved it with the
workspace/host split and the autonomy toggle.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "Default acceptance: By default, agent can do
  anything and everything in the folder that it is operating on. For
  host system, it can read everything but write will require explicit
  permission for any changes outside the workspace. But user should
  have the option to indicate that it can run fully autonomously. This
  is for both clay and st agents." (2026-08-30). Approved for logging:
  "Go ahead and log the decisions as decided already" (2026-08-30).

## Alternatives Considered

1. **Always ask for host writes** — subsumed; that is the default. The
   toggle exists precisely to lift it deliberately.
2. **Ask for everything (pi-style approvals on each action)** —
   rejected: destroys autonomy value inside the workspace where the
   agent already has document authority.
3. **Host writes silently allowed** — rejected: outside-workspace
   changes are the highest-blast-radius action class; they stay gated.

## Rationale and Evidence

- Workspace-scoped freedom matches Clay's document-authority model:
   the daemon owns the workspace; inside it, versions/checkpoints
   provide rollback instead of approvals.
- `ExecutionPolicy` + approval caching (`prism-coding-security`)
   already provides the gating mechanism; this decision sets its
   default scope rather than new machinery.
- The computer-use-linux adoption (same session) explicitly routes
   mutating desktop calls through this policy.

## References

- `roadmap.md` — Phase 1 (default acceptance policy bullet), Phase 5
  (computer-use-linux approvals), resolved item 5.
- `@arnilo/prism-coding-security` — `createCodingApprovalPolicy`,
  `ExecutionPolicy`.
- Decision log 2026-08-21-1758 — server-authoritative documents and
  the trust-domain model behind workspace authority.

## Consequences

- Positive: autonomous runs stay useful inside the workspace; the
  dangerous class (host writes outside workspace) is explicit.
- Risks: a user who flips full autonomy on grants host-write freedom —
  it is a deliberate, visible action, not a default.
- Revisit if: execution surfaces appear that do not route through
  `ExecutionPolicy` (e.g. new MCP tools with side effects).