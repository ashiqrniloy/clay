---
date: 2026-08-30 22:00
status: approved
decision_about: "Worktree/rollback model for clay agent and st"
proposed_by: user
explicitly_approved_by_user: true
---

# Decision: Pi-style session tree extended to the workspace — checkpoints, experiment branches, post-hoc discard

## Decision

Clay adopts pi coding agent's tree model and extends it to workspace
state. Sessions remain single-file JSONL trees (`id`/`parentId` entries)
with pi parity: `/tree` in-place navigation (any earlier entry becomes
the new leaf; all history preserved), branch summaries (LLM summary of
the abandoned path back to the common ancestor), `/fork`, `/clone`,
`/n`, and `--session`/`--fork` CLI equivalents. The extension: document
authority records a **version checkpoint** at every task boundary and
user checkpoint, linked to the session entry active at that moment;
branching from entry E branches both the conversation and the document
version tree at E's checkpoint. For `st` loops, the orchestrator
checkpoints **three layers** per iteration — git commit on the run
branch + document version checkpoint + session tree entry. A wrong
implementation is discardable post hoc (UI or `/tree`): branch the
session from the pre-iteration entry, `git reset` the run branch to the
checkpoint commit, restore the matching document versions — one action,
three layers, consistent. Experiments ride the same mechanism: go
forward from any checkpoint in a scratch branch, keep or discard.
Observational memory keeps a record of dropped iterations (what was
tried, why discarded) even though the code is gone.

## Context

The user wants to go forward from a checkpoint, experiment or implement
in one tree, and discard if required — and to discard a wrong
implementation after a loop iteration post hoc. Pi's tree (verified in
`@earendil-works/pi-coding-agent` docs: `docs/session-format.md` JSONL
tree, `/tree`, `branch(entryId)`, `branchWithSummary`,
`createBranchedSession`, `forkFrom`) is conversation-only; Clay needs
the same concept applied to files and git as well.

## Approval

- Proposed by: user
- Approved by user: Yes
- Approval evidence: "The main idea is that I can go forward from a
  checkpoint and do experimentation or implementation in one tree and
  then discard if required. Also if the AI agent makes a wrong
  implementation in one loop iteration, I can discard it post hoc."
  (2026-08-30). Approved for logging: "Go ahead and log the decisions
  as decided already" (2026-08-30).

## Alternatives Considered

1. **Conversation-only branching (pi as-is)** — rejected: agent edits
   files; rolling back the transcript without the workspace leaves the
   code in the wrong state.
2. **Git-only rollback (reset branch, keep session linear)** — rejected:
   loses the branch summaries and the experiment's conversation context;
   pi's in-place tree with summaries is the better UX.
3. **Copy-on-write workspace snapshots** — rejected: git already
   provides content-addressed snapshots on the run branch; document
   checkpoints provide the editor-side version; a third snapshot system
   would be redundant machinery.

## Rationale and Evidence

- pi session format (`docs/session-format.md`): entries form a tree via
  `id`/`parentId`; `branch(entryId)` moves the leaf; `branch_summary`
  captures abandoned-path context; single-file in-place branching.
- Prism `AgentSession` already exposes `checkout`/`fork`/`clone`
  (readiness table) — the session side exists upstream.
- Clay document authority (versions, leases, dirty buffers; decision
  log 2026-08-21-1758) already versions workspace state — linking a
  checkpoint to a session entry is a join, not a new subsystem.
- `st` loops already git-commit per passed task; the per-iteration
  checkpoint adds the pre-iteration commit so reset has a target.

## References

- `roadmap.md` — Phase 1 (session tree + document checkpoints), Phase 2
  (pi-parity bullet + tree/discard exit-gate drill), Phase 5 (iteration
  checkpoints + post-hoc discard drill), resolved item 8.
- pi docs: `docs/session-format.md`, `/tree`/`/fork`/`/clone` usage.
- Decision log 2026-08-21-1758 — document authority model.

## Consequences

- Positive: safe experimentation; wrong iterations recoverable after
  the fact; nothing ever hard-deleted (abandoned branches keep
  summaries; OM records dropped attempts).
- Risks: three-layer consistency needs the drill gates (Phase 2/5 exit
  gates include them); discarded-branch git objects accumulate until GC.
- Revisit if: multi-agent concurrent edits make per-session document
  checkpoints insufficient.