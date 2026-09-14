# 119 — Editor and Agent Architecture Remediation

Source: `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` (commit
`2864b75`). Scope = that review's Recommended Sequence (items 1–7), with SC-1
executed per the user-approved approach of **2026-09-14: ts-rs codegen from the
DTO layer, not the protocol layer** — `src/protocol/*` (rkyv) is untouched.

## Objectives

- Close every re-verified finding from the 2026-09-14 review: P2-3 dead sandbox,
  version drift, P1-1 eager large-document open, P1-2 long-line position budget,
  P1-3 MCP connect/call timeout conflation, P2-1 unbounded dirty-buffer reads,
  P2-2 non-atomic new-file writes.
- SC-2: extract the connection loop's per-family delivery policy into testable
  helpers (`Delivery` replay enum), loop becomes a thin router.
- SC-1: single-definition webview contract — TS types generated from the DTO
  layer via ts-rs; hand-maintained `types.ts` mirrors deleted; `src/protocol`
  and the DTO projection boundary unchanged in shape and authority.
- SC-6: workspace-scoped agent sessions — server registry keyed by workspace
  root, agent tools resolve workspace from session, frontend agent state scoped
  to the tab runtime.
- SC-3/SC-4: decompose the agent actor and the coding-agent panel; fold the PF-2
  O(n²) transcript scan fix into the panel split.
- Preserve the review's keep-list: budgets centralization, trust boundaries,
  rkyv wire, atomic-save path, DTO projection boundary, checkpoint primitives.

## Expected Outcome

- All Linux gates green (`scripts/check.sh`, `ci.yml`): cargo fmt/check/clippy
  root + clay-desktop, frontend typecheck + suite, agent suite.
- Opening a multi-MiB document delivers a bounded head chunk before any full
  read; 1 MiB single-line position conversion under 15 ms with the existing
  budget test passing (not rebaselined).
- clay-agent MCP servers connect regardless of a small per-call `timeoutMs`;
  slow tool calls still time out.
- `frontend/src/bridge/generated/` is the only TS definition of the webview
  contract; `types.ts` gone (or reduced to branded hand types + re-exports);
  a staleness check in `scripts/check.sh` fails CI on un-regenerated bindings.
- Two tabs over different workspace roots hold independent agent sessions;
  agent tool writes land in the session's workspace, verified by test.
- `handle_connection_loop` reduced to a thin select router; replay-vs-drop
  policy per stream expressed in one enum with unit tests.
- `agent.rs` split into focused modules; `CodingAgentPanel.tsx` split into
  Composer/TranscriptList/ApprovalStrip/InspectorTabs with identical rendered
  output.
- Decision-log entries exist for SC-1 and SC-6.

Out of scope (recorded, not silently dropped): PF-1 lsp-shared
`VersionedDocument` incremental port — latent only, capped at 256 KiB by
`DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` on the analysis route; SC-5 beyond what
SC-1's generated envelope gives; 2026-09-11 review items not re-verified here
(P1-4 js_runtime flake, P1-5 eslint, P3 junk file) — they predate this plan and
remain tracked in that review.

## Tasks

- [ ] Baseline gates and performance baselines on the unmodified tree
  - Acceptance Criteria:
    - Functional: `scripts/check.sh` (or the `ci.yml` equivalent set) passes on the untouched tree; record exit codes.
    - Performance: record the current 1 MiB single-line position-map timing (`frontend/src/editor/position-map.test.ts` P1-2 case, ~29.9 ms known) and `scripts/large-document-smoke.sh` timings as pre-change baselines for comparison in tasks below.
    - Code Quality: no code changes in this task.
    - Security: none introduced.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` (PF/SC findings).
      - `code-reviews/2026-09-11-comprehensive-code-review.md` (P1/P2 evidence).
    - Options Considered:
      - Skip baseline, compare against review numbers only: cheaper but review numbers are one-machine snapshots.
      - Record fresh baselines: makes the perf tasks' before/after deltas trustworthy. (Chosen.)
    - Chosen Approach:
      - Run the full gate set plus the two smoke scripts; store outputs under `code-reviews/2026-09-14-editor-and-agent-architecture-review.md`-adjacent evidence paths or the task evidence log.
    - Files to Create/Edit:
      - None.
    - References:
      - `scripts/check.sh`, `.github/workflows/ci.yml`, `scripts/large-document-smoke.sh`, `scripts/editor-performance-smoke.sh`.
  - Test Cases to Write:
    - None (evidence-recording task).

- [ ] Delete the dead runtime sandbox (P2-3)
  - Acceptance Criteria:
    - Functional: `RuntimeSandboxSupervisor` module and the `clay-runtime-sandbox` binary are removed; no references remain (grep clean); `cargo check --all-targets` green.
    - Performance: build time and binary set shrink; nothing else changes.
    - Code Quality: dead security surface no longer ships; `src/bin/` auto-build wiring updated.
    - Security: removes an unreviveable unsafe path (no heap limit, ignored payload cap) from the shipped set.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P2-3 (0 in-edges, unsafe if revived).
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` backlog.
    - Options Considered:
      - Wire heap limit + payload cap and keep it: keeps speculative process-isolation milestone alive — YAGNI; in-process `DomainRuntime` superseded it.
      - Delete. (Chosen.)
    - Chosen Approach:
      - `git rm` `src/server/runtime_sandbox.rs` and the `src/bin/clay-runtime-sandbox*` source; remove module registration and any build references.
    - Files to Create/Edit:
      - `src/server/runtime_sandbox.rs`: delete.
      - `src/bin/` (sandbox binary): delete.
      - `src/server/mod.rs`: remove module line.
    - References:
      - In-process `DomainRuntime` (128 MB heap, 5 s timeout) is the live mechanism.
  - Test Cases to Write:
    - Existing suites must stay green; no new test for deleted code.

- [ ] Fix clay-agent package.json description drift
  - Acceptance Criteria:
    - Functional: `clay-agent/package.json` description matches the pinned Prism family (0.5.5).
    - Performance: n/a.
    - Code Quality: description no longer contradicts dependencies.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P3.
    - Options Considered:
      - One-line edit vs leaving it: one-line edit. (Chosen.)
    - Chosen Approach:
      - Update the description string only; do not touch dependency pins.
    - Files to Create/Edit:
      - `clay-agent/package.json`: description `0.5.4` → `0.5.5`.
    - References:
      - `decision-logs/2026-09-07-2149-prism-0.5.1-clay-agent-family-pins.md` (pin discipline).
  - Test Cases to Write:
    - `npm test` in clay-agent stays green.

- [ ] P1-1: route large-document open through the chunked protocol
  - Acceptance Criteria:
    - Functional: `workspace::read_file_streamed` no longer eagerly streams the whole file; large opens deliver a bounded head chunk first and continue through the existing `DocumentChunk` progressive path (`onDocumentChunk` in `frontend/src/editor/sync/session.ts`), consistent with `decision-logs/2026-08-25-1253-chunked-document-loading.md`.
    - Performance: `scripts/large-document-smoke.sh` shows head-chunk-first latency for an 8 MiB file within the review's open-latency budget; total transfer unchanged.
    - Code Quality: no second chunking implementation — reuse the existing document-chunk protocol end to end.
    - Security: chunk caps remain server-enforced; no new client trust.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §PF-3.
      - `decision-logs/2026-08-25-1253-chunked-document-loading.md`.
      - `docs/development/editor-performance-review-2026-08-26.md`.
    - Options Considered:
      - Parallel/turbo read: more code, no latency win while head-chunk-first is missing.
      - Reuse existing chunk path for open. (Chosen.)
    - Chosen Approach:
      - Convert the eager path in `src/server/workspace/mod.rs` to emit the head chunk then stream remainder chunks over the existing protocol; verify frontend progressive path handles it without change.
    - Files to Create/Edit:
      - `src/server/workspace/mod.rs`: `read_file_streamed` → chunked emission.
      - `tests/performance_budgets.rs`: add a source-level assertion that the open path references the chunk budget (existing pattern in `workspace_open_path_stays_streamed_and_head_bounded`).
    - References:
      - `src/server/connection/documents.rs` (chunk delivery), `frontend/src/editor/sync/session.ts` (`onDocumentChunk`).
  - Test Cases to Write:
    - Budget test: open of an oversized file emits head first; no single frame exceeds the chunk budget.

- [ ] P1-2: segment long lines in the frontend position index
  - Acceptance Criteria:
    - Functional: the treap chunker in `frontend/src/editor/position-map.ts` splits chunks on byte length (≈128 KiB) in addition to the 64-line rule; conversions on a 1 MiB single-line document are sub-chunk scans.
    - Performance: the existing `position-map.test.ts` 1 MiB single-line budget test (<15 ms) passes for real — the test is the guard and is NOT rebaselined, weakened, or deleted.
    - Code Quality: segmentation is a chunker rule change only; index-vs-fresh-rebuild consistency test still passes.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P1-2 (root cause: 64-line chunking collapses a 1 MiB line into one chunk).
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` backlog.
    - Options Considered:
      - Re-baseline budget and track segmentation later: keeps a documented red ceiling alive.
      - Byte-length segmentation now. (Chosen — the test comment already names it as the fix.)
    - Chosen Approach:
      - Add a max-chunk-bytes rule to the chunker; split on whichever bound hits first; keep conversion math unchanged.
    - Files to Create/Edit:
      - `frontend/src/editor/position-map.ts`: chunker rule.
      - `frontend/src/editor/position-map.test.ts`: no weakening; add a multi-boundary consistency case if coverage is thin.
    - References:
      - Review keep-list: incremental `BytePositionIndex` is also the model for PF-1 later.
  - Test Cases to Write:
    - 1 MiB single-line conversion under 15 ms (existing test, now passing).
    - Random-edit consistency across segmented long lines (extend existing).

- [ ] P1-3: split MCP connect timeout from call timeout in clay-agent
  - Acceptance Criteria:
    - Functional: `clay-agent/src/mcp.ts` keeps `timeoutMs` as the tool-call ceiling and gives connect/handshake its own floor (`max(timeoutMs, 5000)` or a distinct Prism connect timeout if exposed); servers whose process needs ~1 s to spawn + `tools/list` connect even with a small `timeoutMs`.
    - Performance: slow tool calls still time out at `timeoutMs`; connect completes within the floor.
    - Code Quality: both semantics asserted in tests; no log-line-only failure mode.
    - Security: no new authority; allow-list validation untouched.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P1-3 (Prism 0.5.5 bridge passes `limits.callTimeoutMs` as connect pagination timeout).
      - `decision-logs/2026-09-09-1341-mcp-stdio-config-sources-no-approval-gate.md`.
    - Options Considered:
      - Plumb distinct Prism connect timeout: exact if the API exists — verify against the pinned `@arnilo/prism-mcp` 0.5.5 bridge at execution time.
      - Floor of `max(timeoutMs, 5 s)` at our boundary: works regardless of Prism API shape. (Chosen default.)
    - Chosen Approach:
      - Enforce the floor in `clay-agent/src/mcp.ts`; update `mcp-v2.test.ts` to assert both semantics (slow tool call times out AND the server still connects).
    - Files to Create/Edit:
      - `clay-agent/src/mcp.ts`: connect budget.
      - `clay-agent/src/mcp-v2.test.ts`: dual-semantics assertions.
    - References:
      - Fixture server boot ~1 s; sibling passing tests ~1000 ms each (review evidence).
  - Test Cases to Write:
    - `timeoutMs: 200` + slow-boot server → connects; slow tool call → times out.

- [ ] P2-1: cap the dirty-buffer path in `document_read`
  - Acceptance Criteria:
    - Functional: agent `documentRead` on an open (dirty) document returns at most `MAX_READ_BYTES` with a `truncated: true` signal — same cap as the disk path.
    - Performance: no unbounded payload can reach the daemon stdio bridge regardless of resident document size.
    - Code Quality: the reverse-RPC response carries the truncation flag; agent-side byte caps in `document-ops.ts` are defense-in-depth, not the only bound.
    - Security: bounded cross-process payload; no new capability.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P2-1.
    - Options Considered:
      - Rely on agent-side caps: single point of failure at the wrong boundary.
      - Server-side cap + flag. (Chosen.)
    - Chosen Approach:
      - Apply `MAX_READ_BYTES` to the dirty-buffer return in `src/server/agent_documents.rs`; add the flag the response shape already has room for.
    - Files to Create/Edit:
      - `src/server/agent_documents.rs`: dirty-path cap + truncation flag.
      - `src/perf/budgets.rs`: only if the flag plumbing needs a named constant (prefer reusing `MAX_READ_BYTES`).
    - References:
      - Review keep-list: budgets.rs discipline — new ceilings belong there with rationale.
  - Test Cases to Write:
    - Open a document > cap, dirty it beyond cap, `documentRead` returns exactly cap bytes + truncated flag.

- [ ] P2-2: make new-file `document_write` atomic
  - Acceptance Criteria:
    - Functional: new-file creation routes through `contained_new_file_path` + `atomic_write_file` (the pair the workspace open/save paths already use); no direct `tokio::fs::write` remains in the agent write path.
    - Performance: one extra rename+fsync on file creation only.
    - Code Quality: reuses the tested atomic primitive; failure-path temp cleanup inherited.
    - Security: crash mid-write can no longer leave a partial file the agent already reported as success.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-11-comprehensive-code-review.md` §P2-2.
    - Options Considered:
      - fsync-only: still leaves partial-file window on crash.
      - Full atomic path. (Chosen.)
    - Chosen Approach:
      - Replace the direct write for the new-file branch with the existing atomic pair.
    - Files to Create/Edit:
      - `src/server/agent_documents.rs`: new-file branch.
    - References:
      - Review keep-list: "Atomic saves done right" — extend the same primitive.
  - Test Cases to Write:
    - New-file write failure injection (unwritable dir) leaves no partial file and reports failure.

- [ ] SC-2: extract the connection loop's delivery policy
  - Acceptance Criteria:
    - Functional: byte-for-byte wire behavior unchanged; `handle_connection_loop` (`src/server/connection/mod.rs` L496–L1701) reduced to a thin select router delegating to per-family delivery helpers.
    - Performance: no extra copies or hops on the hot loop; the replay-vs-drop policy per stream is expressed in one `Delivery` enum (`Advice` → drop on lag; `State` → replay latest) with unit tests.
    - Code Quality: the replay policy becomes testable without a live connection; extend the `documents.rs` `deliver_parse_update` pattern to the remaining families.
    - Security: connection-scoped session isolation (`ServerMenuSessions`) preserved.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-2.
      - `src/server/connection/documents.rs` (the in-repo extraction pattern to copy).
    - Options Considered:
      - Actor-per-family rewrite: bigger blast radius, no behavior gain.
      - Delivery-helper extraction. (Chosen.)
    - Chosen Approach:
      - Introduce `Delivery` enum; write one helper per broadcast family (typography, editor commands, caret, layout, prefs, tab registry, runtime generation, diagnostics); each helper owns its lag/close policy; the loop body becomes one-liners.
    - Files to Create/Edit:
      - `src/server/connection/mod.rs`: loop slimming.
      - `src/server/connection/documents.rs` or a new `src/server/connection/delivery.rs`: helpers + `Delivery`.
    - References:
      - Existing behavior map: Lagged typography/prefs/tab-registry/caret/layout → replay latest; editor commands/parse → drop; runtime generation → close menu session then replay; broadcast `Closed` → return.
  - Test Cases to Write:
    - Unit: lagged `State` stream replays latest; lagged `Advice` stream drops; `Closed` propagates.

- [ ] SC-1 decision log: ts-rs codegen from the DTO layer
  - Acceptance Criteria:
    - Functional: decision-log entry created under `decision-logs/` (create-decision-log skill format) recording: webview contract source of truth = `src-tauri/src/bridge/dto.rs`; TS generated via ts-rs 12; `src/protocol` (rkyv) untouched; DTO projection boundary preserved as a trust feature, not duplication.
    - Performance: n/a (decision task).
    - Code Quality: rejected alternatives recorded (specta/tauri-specta, JSON-schema+quicktype, conformance-test-only, Vite-time codegen) with reasons.
    - Security: notes that the webview contract stays narrowed by the DTO projections.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-1, §SC-5.
      - Session discussion 2026-09-14 (user approved ts-rs-from-DTO-layer; explicitly not touching `src/protocol`).
      - ts-rs docs via ctx7 `/aleph-alpha/ts-rs` (serde-compat parses `rename_all`, `tag`, `content`, `skip`, `flatten`; export via `cargo test`).
    - Options Considered: as recorded above.
    - Chosen Approach:
      - Log the decision before implementation (this plan is the user's approval of direction; the entry records it).
    - Files to Create/Edit:
      - `decision-logs/2026-09-14-HHMM-ts-rs-generated-webview-contract-from-dto-layer.md` (timestamp at execution).
    - References:
      - Per planning-checklist: after logging, add the reusable guidance line to the smallest relevant `clay-execution` reference (likely `references/protocol-perf.md` or `references/ui.md` → Client Architecture).
  - Test Cases to Write:
    - None (decision task).

- [ ] SC-1: generate the webview TS contract from the DTO layer
  - Acceptance Criteria:
    - Functional: `frontend/src/bridge/generated/` holds TS types generated from the DTO structs; `types.ts` hand-mirrors deleted as covered (leaf DTOs first, `BridgeEnvelope` last); remaining hand file keeps only branded types (`DocumentId`, `MenuSessionId`) and re-exports; frontend typecheck green against generated types.
    - Performance: codegen runs via `cargo test -p clay-desktop` (dev-dependency); frontend builds stay Rust-free.
    - Code Quality: ts-rs 12 as dev-dependency of clay-desktop; `#[derive(TS)] #[ts(export)]` on DTO structs — existing `#[serde(rename_all = "camelCase")]` attrs drive the TS shape via serde-compat (verified against current docs at execution); export fn lives in the existing `runtime_projection_tests` module with `Config` output dir pointing at `frontend/src/bridge/generated/`.
    - Security: no contract widening — generated types mirror exactly what dto.rs serializes today; a shape mismatch surfaced by the migration is a drift bug to fix, not paper over.
  - Approach:
    - Documentation Reviewed:
      - ts-rs 12.0.1 (crates.io, confirmed current): serde-compat default; export via `cargo test`; `Config::new().with_out_dir(...)`; `#[ts(tag)]` for discriminated unions.
      - Verify the exact v12 export/Config API shape against ctx7 `/aleph-alpha/ts-rs` docs during execution — the API moved major versions recently.
      - `src-tauri/src/bridge/dto.rs` (DTO inventory: ~20 structs, ~5 enums incl. tagged `BridgeEnvelope`).
      - `frontend/src/bridge/types.ts` (mirror inventory + branded types to preserve).
    - Options Considered:
      - specta/tauri-specta: geared to Tauri invoke/commands; Clay's frame protocol is the wrong shape.
      - JSON-schema + quicktype: two-step, weaker serde-attr fidelity.
      - Conformance tests without codegen: same upfront work, deletes nothing, keeps three definitions.
      - ts-rs from DTO layer. (Chosen.)
    - Chosen Approach:
      - Incremental migration: leaves → envelope; `types.ts` becomes a re-export barrel, deleted when empty; each step's proof is `npm run check` (typecheck) — mismatch = real drift bug found.
      - Drift guard: `scripts/check.sh` regenerates bindings then fails on `git diff --exit-code -- frontend/src/bridge/generated`.
    - Files to Create/Edit:
      - `src-tauri/Cargo.toml`: `ts-rs = "12"` dev-dependency.
      - `src-tauri/src/bridge/dto.rs`: derives + export test.
      - `frontend/src/bridge/generated/`: generated output (checked in).
      - `frontend/src/bridge/types.ts`: shrink to branded types + barrel, then delete.
      - `scripts/check.sh`: staleness check.
    - References:
      - SC-1 decision-log entry (previous task).
  - Test Cases to Write:
    - `cargo test -p clay-desktop` regenerates bindings; CI diff check fails on stale bindings.
    - Frontend typecheck consumes generated `BridgeEnvelope` with no `any` escape hatches.

- [ ] SC-6 decision log: workspace-scoped agent sessions
  - Acceptance Criteria:
    - Functional: decision-log entry recording the ownership model change: server agent registry keyed by workspace root; tools resolve workspace from session id; frontend agent state scoped to the tab runtime (no module-global session store).
    - Performance: n/a (decision task).
    - Code Quality: migration/compat story recorded (existing single-workspace users keep one session; per-workspace books and checkpoints already keyed by session).
    - Security: tool writes constrained to the session's workspace — an explicit tightening of today's bootstrap-workspace binding.
  - Approach:
    - Documentation Reviewed:
      - `code-reviews/2026-09-14-editor-and-agent-architecture-review.md` §SC-6.
      - `decision-logs/2026-09-11-2331-workspace-agent-tab-model-and-launcher-landing.md` (a tab is one workspace + one agent).
      - `decision-logs/2026-08-30-2200-pi-tree-model-extended-to-workspace-checkpoints-and-discard.md` (session-keyed checkpoint primitives).
    - Options Considered:
      - Keep process-global agent + route by active tab: leaves wrong-workspace writes and cross-tab state bleed.
      - Workspace-scoped sessions. (Chosen — matches the approved tab model.)
    - Chosen Approach:
      - Log before implementation; the entry names the invariant: one workspace root → one agent session → tools bound to that root.
    - Files to Create/Edit:
      - `decision-logs/2026-09-14-HHMM-workspace-scoped-agent-sessions.md` (timestamp at execution).
    - References:
      - Per planning-checklist: update the smallest relevant `clay-execution` reference if guidance is reusable.
  - Test Cases to Write:
    - None (decision task).

- [ ] SC-6 (server): workspace-keyed agent registry and tool resolution
  - Acceptance Criteria:
    - Functional: the server agent registry keys sessions by workspace root; agent tool execution resolves the workspace from the session instead of the bootstrap workspace; existing single-workspace flows behave identically.
    - Performance: registry lookup O(1) per session; no per-request workspace scanning.
    - Code Quality: no copy of workspace state into agent state — resolution goes through the existing workspace authority (`src/server/workspace/mod.rs`).
    - Security: a session's tools cannot address files outside its workspace root (existing containment checks now anchored to the right root).
  - Approach:
    - Documentation Reviewed:
      - Review §SC-6; `src/server/agent.rs` (registry, `apply_book_event`, `book_snapshot`), `src/server/agent_documents.rs` (tool execution/reverse RPC), `src/server/mod.rs` L723 (registration).
    - Options Considered:
      - Per-tab agent instances at the transport layer: duplicates sessions per view; the workspace root is the real identity.
      - Workspace-root keying. (Chosen.)
    - Chosen Approach:
      - Key the session registry by workspace root; thread the resolved root through tool execution; keep the book/checkpoint keying unchanged.
    - Files to Create/Edit:
      - `src/server/agent.rs`: registry keying + session→workspace resolution.
      - `src/server/agent_documents.rs`: tool paths resolve workspace from session.
      - `src/server/mod.rs`: registration wiring if the bootstrap binding changes shape.
    - References:
      - SC-6 decision-log entry (previous task).
  - Test Cases to Write:
    - Two registries (two roots), two sessions: tool write from session A lands under root A; B under B.
    - Session resume restores into the same workspace key.

- [ ] SC-6 (frontend): scope agent session state to the tab runtime
  - Acceptance Criteria:
    - Functional: agent session state lives on the tab runtime (`frontend/src/shell/workspace-controller.ts` `TabRuntime`) instead of the process-global store (`frontend/src/agent/state.ts`); switching tabs switches the agent view's session; closing a tab disposes its agent session state.
    - Performance: no cross-tab re-render cascades; per-tab stores follow the existing dependency-free observable pattern.
    - Code Quality: no module-global mutable agent state remains; AG-UI custom-event handling (`clay.*`) routes per-session.
    - Security: no session transcript bleed across tabs.
  - Approach:
    - Documentation Reviewed:
      - Review §SC-6; `frontend/src/agent/state.ts`, `frontend/src/agent/TauriClayAgent.ts`, `frontend/src/shell/workspace-controller.ts`, `frontend/src/shell/workspace-envelope.ts`.
      - `.agents/skills/clay-execution/references/ui.md` → Client Architecture (server authority, stable-ID React reconciliation, narrow Tauri capabilities).
    - Options Considered:
      - Keep global store + active-tab indirection: the status quo's leak, dressed up.
      - Per-tab runtime ownership. (Chosen.)
    - Chosen Approach:
      - Move session ownership into `TabRuntime`; the panel reads from its tab's runtime; disposal on tab close.
    - Files to Create/Edit:
      - `frontend/src/agent/state.ts`: store scoped per session instance.
      - `frontend/src/shell/workspace-controller.ts`: runtime owns agent session.
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: consume from runtime prop/context.
    - References:
      - SC-6 decision-log entry.
  - Test Cases to Write:
      - Tab A + tab B over different roots: transcript/state isolated; switching preserves each.
      - Closing a tab disposes its agent state (no leaked subscriptions).

- [ ] SC-6 (verification): multi-tab agent integration pass
  - Acceptance Criteria:
    - Functional: full flow on a real Linux build — two workspaces open, one agent tab each, prompts, a file write per session, resume per session; writes land in the right roots.
    - Performance: startup and session-switch latency within baseline (task 1) ± noise.
    - Code Quality: covered by automated tests where possible; manual pass records the rest.
    - Security: verified no cross-root tool addressing.
  - Approach:
    - Documentation Reviewed:
      - Review §SC-6; test-plan module map (`test-plan/index.md`).
    - Options Considered:
      - Automated-only: the ownership bug class is cross-component; a live pass is the cheap insurance.
      - Automated + live. (Chosen.)
    - Chosen Approach:
      - Live two-workspace session; record evidence paths in task completion.
    - Files to Create/Edit:
      - `test-plan/` (numbered steps added in the manual-test-plan task below).
    - References:
      - Review keep-list: checkpoint primitives are the resume backbone.
  - Test Cases to Write:
    - End-to-end two-tab isolation + resume scenario.

- [ ] SC-3: split the server agent actor into focused modules
  - Acceptance Criteria:
    - Functional: pure mechanical split — `agent/book.rs` (apply/snapshot/load), `agent/run.rs` (run pipeline, approvals), `agent/mcp.rs` (inventory) extracted from `src/server/agent.rs`; behavior identical; all agent tests green.
    - Performance: no call-path changes.
    - Code Quality: follows the existing `agent_documents.rs`/`agent_checkpoints.rs` extraction pattern; no logic edits beyond visibility.
    - Security: boundary unchanged.
  - Approach:
    - Documentation Reviewed:
      - Review §SC-3; `src/server/agent.rs` (L496–L1701 loop is connection, agent actor is ~L3400+), existing extraction precedent.
    - Options Considered:
      - Split first, then SC-6: double-churn on hot files.
      - Split after SC-6 lands (behavioral change on final state). (Chosen — review's "opportunistic" guidance.)
    - Chosen Approach:
      - Module moves only; `git mv`-style history preserved where practical.
    - Files to Create/Edit:
      - `src/server/agent.rs` → `src/server/agent/` (book.rs, run.rs, mcp.rs; mod wiring).
    - References:
      - Review §SC-3.
  - Test Cases to Write:
    - None new; existing suites are the guard.

- [ ] SC-4: decompose CodingAgentPanel (fold PF-2 in)
  - Acceptance Criteria:
    - Functional: `CodingAgentPanel.tsx` (~2300 lines) split into `Composer` (slash + @-mention + effort), `TranscriptList` (+ the PF-2 previous-agent scan fixed as one reverse-pass memo, replacing the per-box `slice().reverse().find()` O(n²)), `ApprovalStrip`, `InspectorTabs`; rendered output and behavior identical; existing panel tests green unchanged.
    - Performance: transcript render drops from O(n²) to O(n) on the agent-label scan; render-count parity verified on a representative session.
    - Code Quality: each extracted component independently testable; state machines unchanged inside components.
    - Security: approval flow (`runResume`) semantics untouched.
  - Approach:
    - Documentation Reviewed:
      - [`DESIGN.md`](../../DESIGN.md) (normative Quiet Instrument language; §12 agent surface; §14 retired patterns).
      - `.agents/skills/clay-execution/references/ui.md` (client architecture, skill routing), `references/components.md`, `references/tokens.md` (catalogs), `docs/reference/ui-components.md`.
      - Review §SC-4, §PF-2.
    - Options Considered:
      - Prototype gate: NOT triggered — this is a structural refactor with byte-identical rendered output; no new surface, state, or aesthetic change. Conformance target: existing approved set `design-artifacts/approved/quiet-instrument-migration/`. Any deviation surfaced during the refactor re-enters the prototype loop.
      - Extract everything at once vs leaf-first: leaf-first (Composer last — most coupled). (Chosen.)
    - Chosen Approach:
      - Extract leaf components with props-in/props-out; keep the panel as the composition root; fix the PF-2 scan inside `TranscriptList`.
    - Files to Create/Edit:
      - `frontend/src/coding-agent/CodingAgentPanel.tsx`: shrink to composition root.
      - `frontend/src/coding-agent/` new: `Composer.tsx`, `TranscriptList.tsx`, `ApprovalStrip.tsx`, `InspectorTabs.tsx` (tentative names).
      - `frontend/src/coding-agent/CodingAgentPanel.test.tsx`: unchanged expectations; component tests added where extracted logic is non-trivial (mention tokenizer, effort chord).
    - References:
      - Review §SC-4, §PF-2; DESIGN.md §12 (the agent view's title is the agent-type picker; Files tab is session file history).
  - Test Cases to Write:
    - Component-level: mention-token parser edge cases; effort-chord cycling; approval allow/deny payloads.

- [ ] Perform visual screenshot and accessibility review of changed UI
  - Acceptance Criteria:
    - Functional: real Linux GUI build exercised over the agent panel (default, streaming, approval, empty transcript, inspector tabs) and two-tab agent sessions; screenshots per state stored under a named artifact path; findings recorded.
    - Performance: narrow + wide layouts captured where layout can change.
    - Code Quality: comparison against `design-artifacts/approved/quiet-instrument-migration/` surface by surface; every deviation recorded with disposition (fixed to match or explicitly re-approved); zero unrecorded deviations.
    - Security: n/a.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-08-14-0200-mandatory-ui-visual-and-accessibility-review.md`.
      - `.agents/skills/clay-execution/references/planning-checklist.md` (Visual and Accessibility Review Duty).
      - `scripts/capture-ui-review.sh`.
    - Options Considered:
      - Source-inspection only: explicitly forbidden by the duty.
      - Live review with screenshots + `computer-use-linux` `get_app_state` a11y pass. (Chosen.)
    - Chosen Approach:
      - Exercise changed states, inspect the a11y tree, verify keyboard-only flow over composer/completions/approval strip; record blocker verbatim if GUI tooling is unavailable rather than claiming a pass.
    - Files to Create/Edit:
      - Evidence under `code-reviews/screenshots/` or task evidence paths.
    - References:
      - SC-4, SC-6 tasks.
  - Test Cases to Write:
    - None (review task; evidence in completion record).

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: review all Rust public functions and behaviors introduced/changed by this plan; confirm none requires a new public Clay JS API (plan changes internal architecture, protocol delivery, and agent session ownership — no new user-programmable capability); any function that should not be exposed is private/`pub(crate)`.
    - Performance: n/a.
    - Code Quality: registry/docs coverage checks (`cargo test` gates) green.
    - Security: no implicit authority exposure.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/js-api.md`.
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`.
    - Options Considered:
      - Skip: the clay.md requirement makes this task mandatory per plan; it closes as "verified, none needed" with evidence.
      - Verify-only task. (Chosen.)
    - Chosen Approach:
      - Inventory changed public Rust functions (SC-2 helpers, SC-6 registry, P2 paths); confirm privacy or JS API coverage.
    - Files to Create/Edit:
      - None expected; edits only if an exposure gap is found.
    - References:
      - `src/packages/manifest.rs` `RESERVED_CORE_API_DOMAINS` naming rules if anything new surfaces.
  - Test Cases to Write:
    - None expected; existing `cargo test` registry gates are the check.

- [ ] Create or verify Clay configuration surfaces
  - Acceptance Criteria:
    - Functional: review changed behaviors (chunked open, MCP connect floor, workspace-scoped agents, delivery policy); confirm none introduces a user-facing configuration option; record the review conclusion.
    - Performance: n/a.
    - Code Quality: if any option is found to be needed (e.g., an MCP connect-timeout override), it lands as a documented Clay JS API with `custom_properties` coverage — not an undocumented key.
    - Security: configuration never implicitly grants workspace/network/shell authority.
  - Approach:
    - Documentation Reviewed:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`.
    - Options Considered:
      - Add config keys preemptively for new budgets: speculative — the budgets are internal constants with rationale in `budgets.rs`.
      - Verify-only. (Chosen.)
    - Chosen Approach:
      - Confirm zero new config surface; document that agent workspace scoping is structural (follows the tab model), not user-configured.
    - Files to Create/Edit:
      - None expected.
    - References:
      - `src/perf/budgets.rs` (internal ceilings stay internal).
  - Test Cases to Write:
    - None expected.

- [ ] Execute and update the manual test plan (test-plan/)
  - Acceptance Criteria:
    - Functional: identify affected modules in `test-plan/index.md`; execute relevant steps on a real Linux build, recording pass/fail against numbered steps; add new numbered steps for new user-visible behavior (chunked large-file open, MCP servers connecting under small `timeoutMs`, per-workspace agent sessions/two-tab isolation, agent resume per workspace).
    - Performance: large-file open latency step added with its budget.
    - Code Quality: no existing step weakened or deleted; failures are defects or explicitly documented ceilings.
    - Security: multi-tab agent isolation steps include the cross-root negative check.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md` (module map + coverage matrix).
    - Options Considered:
      - Automated-only: plan changes user-visible flows; the manual duty applies.
      - Execute + extend. (Chosen.)
    - Chosen Approach:
      - Map plan tasks → test-plan modules; add steps; run them on the final build.
    - Files to Create/Edit:
      - `test-plan/03-files-and-workspace.md` (chunked open, agent workspace isolation — placement per index).
      - Affected module files per the coverage matrix.
      - `test-plan/index.md` if the matrix changes.
    - References:
      - SC-6 verification task (live pass feeds this).
  - Test Cases to Write:
    - The added numbered steps are the deliverable.

- [ ] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: The project code wiki is updated after all implementation tasks are complete, or explicitly verified as unchanged for non-code work.
    - Performance: Wiki updates add no runtime work and document performance-relevant implementation details changed by the plan.
    - Code Quality: Wiki pages explain what changed code does, how it works, invariants/tradeoffs, source/test paths, examples where useful, and links from the master wiki index.
    - Security: Wiki pages document touched security boundaries, permissions, validation, secrets handling, or external authority without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-execution/references/docs-as-code.md`: wiki workflow, quality bar, and archive policy.
    - Options Considered:
      - Update after each task: more granular, but noisy and likely to churn.
      - Update once after tests pass: keeps docs aligned with final code. (Chosen.)
    - Files to Create/Edit:
      - `docs/wiki/index.md`: navigation links for changed implementation areas.
      - `docs/wiki/**`: implementation wiki pages for changed code (delivery policy, generated bridge contract, workspace-scoped agents, decomposed modules).
  - Test Cases to Write:
    - Manual wiki review: the master index links relevant pages and updated pages explain what changed implementation does and how it works.

## Compromises Made

- Known constraint at planning time: PF-1 (lsp-shared `VersionedDocument` O(n)-per-edit) is deferred — capped in practice by `DOCUMENT_ANALYSIS_MAX_DOCUMENT_BYTES` (256 KiB) on the analysis route; upgrade path (port the incremental `BytePositionIndex`) recorded in the review.
- SC-5 (single AG-UI envelope end-to-end) is limited to what SC-1's generated contract gives; full envelope unification across stdio/rkyv/JSON hops stays out of scope.

## Further Actions

- To be filled after task completion with improvements, rationale, and priority.