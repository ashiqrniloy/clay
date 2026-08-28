---
date: 2026-08-27 01:59
status: approved
decision_about: "Resume Plan 099 with server-authoritative syntax after Lezer rejection"
proposed_by: "user"
explicitly_approved_by_user: true
---

# Decision: Resume Plan 099 on the server-authoritative path

## Decision

Stock client-local Lezer parsing is rejected for this performance cycle after the
grammar freshness gate found recovery nodes in the frozen modern Rust and
TypeScript probes. Continue Plan 099 with server-side per-document Tree-sitter
sessions and atomic viewport patches; keep CodeMirror responsible for local text,
selection, history, viewport, and inert presentation only.

Clean the branch by removing the disposable Lezer dependencies, editor selector,
recorder integration, and spike runner. Preserve the dated fail-fast report,
decision history, and Plan 100 evidence.

## Context

The ordered Plan 100 run passed all current-language fixtures but failed before
WebKitGTK, viewport, memory, four-pane, and reproducibility gates. The first
failure was `rust-modern` with `@lezer/rust` 1.0.2 and four recovery nodes; the
modern TypeScript probe also recorded two recovery nodes with
`@codemirror/lang-javascript` 6.2.5. The machine-readable summary marks all
later gates and 90 planned scenarios not applicable.

## Approval

- Proposed by: User, after the fail-fast condition was not met.
- Approved by user: Yes.
- Approval evidence: User stated, "Given the condition did not meet, I will now continue with plan 099" and then requested cleanup so Plan 099 could continue.

## Alternatives Considered

1. **Return to the pre-spike branch** — rejected because the current spike branch
   and `arch/tauri_react` point to the same commit, while the spike work is still
   uncommitted; switching would not clean the working tree.
2. **Continue carrying Lezer code and dependencies** — rejected because the
   candidate failed its first hard gate and would leave a rejected parser path in
   the production editor dependency graph.
3. **Preserve evidence but clean the production tree, then resume Plan 099** —
   selected. This retains auditability without promoting disposable code.

## Rationale and Evidence

- `plans/100-Client-Local-Parsing-Spike-and-Parser-Placement-Decision.md`
  requires stopping on the first hard failure and re-planning Plan 099 in place.
- `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
  already approves server-side package-flexible syntax, one bounded session per
  document/grammar, and atomic viewport patches.
- The current branch and `arch/tauri_react` both resolve to `386f4db`; the
  worktree contains uncommitted spike changes, so cleanup in place is the lowest-
  risk workflow.
- The Lezer editor integration, direct language dependencies, local syntax-owner
  filter, and spike runner are disposable. The dated report and decision records
  are retained as historical evidence.

## References

- `plans/099-Clay-Editor-Performance-Overhaul.md`
- `plans/100-Client-Local-Parsing-Spike-and-Parser-Placement-Decision.md`
- `docs/development/client-local-parsing-spike-2026-08-26.md`
- `target/perf/client-local-parsing/summary.json`
- `target/perf/client-local-parsing/grammar.json`
- `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
- `decision-logs/2026-08-26-2137-lezer-fail-fast-before-server-syntax-overhaul.md`
- `.agents/skills/project-patterns/references/protocol-and-performance.md`

## Consequences

- Plan 099 is reactivated and continues with its first remaining implementation
  task after parser-dependent wording is updated.
- No direct Lezer language package, local parser selector, client parser
  recorder, or duplicate base syntax owner remains in the frontend production
  dependency/source tree; CodeMirror's existing parser-runtime dependencies
  remain untouched.
- Fail-fast report and plan history remain available for future audit; another
  client parser spike requires a new metric-backed decision after the server
  overhaul misses its approved gates.
- Frontend-worker parsing remains closed unless completed server-side work misses
  approved metrics and traces attribute the remaining delay to server/bridge
  placement.
