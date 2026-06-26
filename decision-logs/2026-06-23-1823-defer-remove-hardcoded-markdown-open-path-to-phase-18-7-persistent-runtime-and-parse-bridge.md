---
date: 2026-06-23 18:23
status: approved
decision_about: "Defer removing the hardcoded Markdown open path to Phase 18.7 (persistent runtime + JS ParseHandler bridge) because the generic live-parse path it must route through does not exist yet"
proposed_by: "agent"
explicitly_approved_by_user: true
---

# Decision: Defer removing the hardcoded Markdown open path to Phase 18.7 (persistent runtime + JS ParseHandler bridge)

## Decision

Plan 030 task "Remove hardcoded Markdown open path in favor of generic mode activation" is **deferred**. It cannot be completed as a cleanup task because the generic package/mode/parse-coordinator path it must route through does not exist in production today. The work is tracked as a new **Phase 18.7** in `roadmap.md` (persistent shared JS runtime + a JS→Rust `ParseHandler` bridge), which is the direct continuation of the now-completed Phase 18.6 generic `loadPackage("@clay/*")` resolver and a prerequisite for the existing Phase 19 hot-reload semantics. The Plan 030 task is marked deferred with a decision-log-backed rationale, not silently dropped.

## Context

`code-reviews/2026-06-21-current-implementation-review.md` finding **P2-3** (Architecture + Performance, the lowest priority tier) flagged that selected Markdown file open in `src/server/connection.rs::selected_file_open_followup_messages` (and its helpers `evaluate_markdown_open`, `create_markdown_open_runtime_root`, `unique_markdown_open_runtime_root`, `markdown_open_init_source`) creates a temporary runtime root, copies the Markdown `dist` JS files (`index.js`, `load.js`, `parser.js`), writes a generated `init.js` with a bounded 64 KiB UTF-8 text prefix, evaluates a fresh JS runtime, then removes the temp directory. The recommended fix is to "route open-time mode activation and parse decoration publication through the generic package/mode/parse primitives" and keep Markdown-specific parser logic inside the package.

Investigation during Plan 030 execution found that the generic live-parse path the task depends on does **not** exist in production. Three structural gaps block a safe in-task completion:

1. **No persistent runtime.** `ClayJsRuntimeService::evaluate_module_on_runtime` (`src/server/js_runtime.rs`) constructs a fresh `deno_core::JsRuntime` per call and drops it when the evaluation returns. No runtime survives across operations. The startup configuration runtime (`~/.config/clay/init.js` → `await loadPackage("@clay/markdown")`) is also short-lived and is dropped after `apply_runtime_outputs`. There is therefore no persistent runtime against which a generic mode activation could register live handlers at file-open time.

2. **No JS→Rust parse bridge.** `ParseCoordinator::register_handler` (`src/server/parse_coordinator.rs`) takes a real `impl ParseHandler` trait object, but it is only ever called from tests (`tests/parse_coordinator.rs`, `tests/markdown_mode.rs`). In production, `op_clay_parse_register_parse_handler` (`src/server/ops/parse.rs`) records **metadata only** (`ParseHandlerMeta { package_prefix, mode_id }`) and `reject_executable_handler` explicitly rejects executable `handler`/`callback`/`onParse`/`function` callbacks. Nothing wires a JS parse function into `ParseCoordinator::schedule_parse`. The coordinator's `schedule_parse`/`next_update`/`IncrementalParseUpdate` machinery is test-only scaffolding today.

3. **Markdown decorations are produced only by the per-file fresh runtime.** `publishMarkdownDecorations` (`packages/markdown/dist/parser.js`) runs the real `markdown-it` parse inside the ephemeral runtime built by `connection.rs`. There is no other producer of Markdown decorations in production. Removing the special path without building a replacement leaves document decorations with no source.

Satisfying Plan 030's acceptance criteria (remove the temp runtime + dist copy, route through the generic package/mode/parse coordinator path, no fresh JS runtime per open, no Markdown branches in connection handling) therefore requires building the persistent runtime and the JS `ParseHandler` bridge and wiring them into the document open/edit flows. That is a foundational architecture change, not a cleanup, and it is the same class of work as the Phase 18.6 `loadPackage` resolver that was itself deferred (and later built) under `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md`.

## Approval

- Proposed by: agent (during Plan 030 "Remove hardcoded Markdown open path" task execution).
- Approved by user: Yes.
- Approval evidence: The user asked "How important is it that we do this task with everything else that needs to be done," the agent explained it is P2 (architecture + performance), lowest review tier, with no security/correctness/data-loss impact and a clear deferral precedent, and the user replied: "Yes write the decision log and update the plan to defer. But also add a new phase in the @roadmap.md as Phase 19 so that it is addressed next with sufficient details and context as described above."

## Alternatives Considered

1. **Complete the task inside Plan 030 as a cleanup.** — Rejected. Requires building the persistent runtime + JS `ParseHandler` bridge + document-flow wiring, which is a security-relevant authority expansion (a long-lived server runtime that holds package closures and invokes JS on the edit/open hot path). That warrants its own plan, security review, decision log, and dedicated tests — the same bar Phase 18.6 was held to — not a sub-task of a code-review remediation plan. Faking completion by relocating the temp runtime (e.g. into a generic helper) would move complexity without removing the per-open V8 spawn and would not satisfy the criteria.
2. **Ship a partial refactor (extract the Markdown path into a generic "open-time activation" helper, keep the fresh runtime for now).** — Rejected. It does not remove the per-open runtime spawn, does not remove the dist-file copy, and adds an abstraction layer with one implementation (Markdown), violating the project's no-mode-branches and no-one-implementation-abstraction conventions (`.agents/skills/project-patterns/references/mode-primitive-first.md`). It would be churn without benefit.
3. **Build the persistent runtime + JS parse bridge as an unnumbered task inside Plan 030.** — Rejected. Plan 030 is a code-review remediation plan; the bridge is a new subsystem that changes runtime lifecycle, authority, and the edit hot path. It needs its own phase with its own primitive review, per `.agents/skills/project-patterns/references/mode-primitive-first.md`.
4. **Defer with a decision log and track the work as a new roadmap phase addressed next.** — Selected. It matches the 2026-06-15 deferral precedent, preserves the generic-mode-activation target, records the three structural gaps as evidence, and puts the foundational work where it belongs (its own phase) so it is addressed next with sufficient context rather than folded into a remediation plan.

## Rationale and Evidence

- **Review priority is P2, the lowest tier.** `code-reviews/2026-06-21-current-implementation-review.md` classifies P2-3 as Architecture + Performance only. Every P0/P1 security item in Plan 030 is already complete. The remaining Plan 030 work (clippy/lint gate, boxed diagnostics, public API/dependency trim, COM safety comments, Clay maintenance) is independent of this task and tractable now.
- **No security, correctness, or data-loss impact.** The Markdown open path runs inside the same deny-by-default V8 sandbox, under the 5 s wall-clock timeout added in Plan 030 task 6 (`JS_RUNTIME_EVALUATION_TIMEOUT_MS`, surfaced as `clay.runtime.timeout`), with a bounded 64 KiB `init.js` text window. It produces read-only parse output (decorations/manifest) and grants no new authority. It works as designed today.
- **The cost is bounded and deferred.** The per-open cost (fresh isolate + dist copy + init write + eval + temp cleanup, roughly hundreds of milliseconds plus disk I/O) fires once per Markdown file open, not on the editing hot path. It is a performance annoyance, not a regression.
- **The work is the direct continuation of Phase 18.6.** Phase 18.6 shipped the constrained generic `loadPackage("@clay/*")` resolver and first-party module-loader bridge. The remaining gap to generic open-time mode activation is exactly the persistent runtime and the JS `ParseHandler` bridge that 18.6 deliberately left out (its "Carried-forward items" defer hot reload to Phase 19 but do not cover live parse wiring, which is the gap surfaced here).
- **Precedent.** `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` deferred the generic resolver on the same basis: the work "cannot be a thin wrapper," "warrants its own plan, security review, decision log, and dedicated tests," and was tracked as Phase 18.6 rather than folded into a Markdown replan. This decision applies the same standard to the runtime/parse half.

## Numbering note

The new phase is added as **Phase 18.7**, not "Phase 19." The literal "Phase 19" was requested, but it is already taken in the roadmap by "Hot Reload and Behavior Update Semantics" and in `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`, with 22 cross-references across `plans/`, `decision-logs/`, and `docs/`. Renumbering the existing 19–23 phases would churn 40+ references with real inconsistency risk. Phase 18.7 follows the roadmap's established decimal-sub-phase convention (16.5 between 16/17; 18.1–18.6 as decimal sub-phases of 18), is semantically exact (it is the continuation of 18.6), and is a prerequisite for the existing Phase 19 hot-reload work. A literal renumber to 19 can be done later if the user explicitly wants it.

## References

- `code-reviews/2026-06-21-current-implementation-review.md` — P2-3 "Markdown open path is hardcoded and expensive" (Architecture + Performance).
- `plans/030-Code-Review-Security-Architecture-and-Quality-Fixes.md` — task "Remove hardcoded Markdown open path in favor of generic mode activation" (now deferred).
- `src/server/connection.rs` — `selected_file_open_followup_messages`, `evaluate_markdown_open`, `create_markdown_open_runtime_root`, `unique_markdown_open_runtime_root`, `markdown_open_init_source`.
- `src/server/js_runtime.rs` — `evaluate_module_on_runtime` (fresh per-call runtime), `ClayModuleLoader` (deny-by-default module boundary), `CLAY_FACADE_PACKAGES`/`loadPackage` resolver.
- `src/server/parse_coordinator.rs` — `ParseHandler` trait, `register_handler`, `schedule_parse` (production-unreachable today).
- `src/server/ops/parse.rs` — `op_clay_parse_register_parse_handler` (metadata-only + `reject_executable_handler`).
- `packages/markdown/dist/load.js` and `packages/markdown/dist/parser.js` — package load entry and `publishMarkdownDecorations`.
- `decision-logs/2026-06-15-1015-defer-generic-loadpackage-first-party-resolver.md` — the deferral precedent this decision mirrors.
- `decision-logs/2026-06-16-1526-generic-first-party-package-loadentry-module-bridge.md` — the Phase 18.6 authority expansion this new phase continues.
- `.agents/skills/project-patterns/references/mode-primitive-first.md` — no-mode-specific Rust branches; primitive-first planning.
- `roadmap.md` — Phase 18.6 (generic resolver, complete) and Phase 19 (hot reload, depends on this work).

## Consequences

- The Markdown file-open path keeps its per-open fresh runtime and dist copy for now. This is a known, bounded P2 performance/architecture cost, documented here and in the roadmap.
- Plan 030's remaining tasks (clippy/lint gate, boxed diagnostics, public API/dependency trim, COM safety comments, Clay maintenance) proceed independently; none are blocked by this deferral.
- **Phase 18.7 is added to `roadmap.md`** as the home for: (a) a persistent/shared server-side JS runtime that survives across document open/edit operations, (b) a constrained JS→Rust `ParseHandler` bridge that lets a resolver-validated package register a real parse callback into `ParseCoordinator::schedule_parse` without accepting arbitrary executable callbacks, (c) open-time generic mode activation that reuses the Phase 18.6 `loadPackage` path instead of spawning a fresh runtime, and (d) removal of the Markdown-specific `connection.rs` branch once the generic path produces equivalent behavior/manifest/SDUI/decorations.
- Phase 18.7 must ship as one coherent, security-reviewed unit with its own plan, primitive review, decision log, and dedicated tests, matching the Phase 18.6 bar, because it expands the live runtime surface (long-lived runtime holding package closures; JS invoked on the document edit/open hot path).
- Phase 19 hot-reload semantics remain dependent on Phase 18.7's persistent runtime; this decision makes that dependency explicit in the roadmap.
- No new filesystem, network, shell, AI, WASM, raw-op, native-widget, client-JS, package-enable/disable, or package-manager execution authority is granted by this deferral. Authority expansion happens only when Phase 18.7 ships, under its own decision log.
