# Clay Editor Performance Overhaul

> **Status: active and re-planned.** Plan 100 rejected stock client-local Lezer parsing at its grammar freshness gate; the dated report and final parser-placement decision are `docs/development/client-local-parsing-spike-2026-08-26.md` and `decision-logs/2026-08-27-0159-resume-server-authoritative-editor-performance.md`. Continue this plan on the server-authoritative Tree-sitter-session path. Do not promote disposable Lezer spike code or dependencies.

## Objectives

- Eliminate document-sized browser main-thread work from typing, scrolling, and decoration application.
- Keep one frontend document owner per pane and remove duplicate chunk transfer, stale shadows, and programmatic load history.
- Replace fragmented viewport syntax traffic with one explicit, versioned, atomic patch contract.
- Move mode activation, parse-window extraction, and native parsing off the connection loop and Tokio worker threads.
- Bound CPU, memory, queues, cache retention, React commits, and parser concurrency per document.
- Prove improvement on real Linux WebKitGTK hardware across file sizes, line shapes, languages, panes, and degraded rendering.

## Expected Outcome

- Normal typing performs no full-document scan, React editor-tree commit, CodeMirror compartment reconfiguration, Tauri wait, server wait, or parser work.
- One pane owns one current CodeMirror `Text`; detached snapshots are bounded and no obsolete global session receives production traffic.
- Viewport syntax uses request IDs and one complete patch, including explicit empty completion; no timer or unrelated fold/diagnostic event stands in for acknowledgement.
- Frontend decorations, diagnostics, and folds update only covered ranges and retain only visible plus bounded overscan state.
- Each document/grammar has at most one active syntax job on a bounded blocking executor; newer versions/viewports coalesce latest-wins.
- File head/chunk transfer remains bounded as implemented by Plan 098, while real WebKit first-text/ready/typing/scroll metrics meet the targets in `docs/development/editor-performance-review-2026-08-26.md`.
- Client-local parsing remains rejected for this performance cycle after Plan 100's grammar failure; reconsider it only if completed server sessions miss approved metrics and traces attribute the remaining delay to server/bridge placement.

## Authority and Phase Boundaries

- Server remains canonical owner of documents, versions, leases, file/workspace access, package/mode selection, grammar provenance, and semantic/LSP authority.
- CodeMirror owns current frontend text, selection, history, viewport, incremental byte-position index, and inert visible render data.
- React owns chrome and coarse metadata only.
- Tauri Rust remains a narrow validated transport/OS bridge and gains no document shadow or parser authority.
- Package JavaScript, filesystem work, V8 evaluation, parser execution, AI, and Tauri IPC remain outside browser input/render handlers.
- No package-facing component kind, token, style variable, permission, raw CSS surface, or native handle is added.

## Tasks

- [x] Audit current editor performance flow and review existing primitives
  - Acceptance Criteria:
    - Functional: Review traces open, progressive load, typing, viewport changes, syntax parsing, decorations, diagnostics, folds, Tauri delivery, and React ownership end-to-end against the current Plan 098 working tree.
    - Performance: Review records at least one real hot-path measurement and distinguishes measured cost from structural hypotheses.
    - Code Quality: Findings identify root causes, current tests that miss them, realistic options, and minimum working architecture rather than proposing unrelated abstractions.
    - Security: Recommended changes preserve server authority, inert render data, package provenance, and bounded local IPC.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/rust-best-practices/references/chapter_01.md`
      - `.agents/skills/rust-best-practices/references/chapter_02.md`
      - `.agents/skills/rust-best-practices/references/chapter_03.md`
      - `.agents/skills/rust-best-practices/references/chapter_06.md`
      - `.agents/skills/rust-best-practices/references/chapter_09.md`
      - `docs/wiki/modules/react-codemirror-editor.md`
      - `docs/wiki/modules/parse-coordinator.md`
      - `docs/wiki/modules/decoration-transport.md`
      - `docs/wiki/flows/frontend-edit-synchronization.md`
      - `docs/wiki/flows/document-chunked-loading.md`
      - Context7 `/websites/codemirror_net`: transactions, state fields, visible ranges, view update phases, and position mapping.
      - Exact local crates: `crop 0.4.3`, `tree-sitter 0.25.10`, `tokio 1.52.2` from `cargo metadata` / `cargo tree`.
    - Options Considered:
      - Attribute stalls to large-file wire transfer only: rejected; Plan 098 bounds transfer but browser hot paths still scale with document/retained state.
      - Replace server syntax immediately with client parser: rejected before optimizing measured shared bottlenecks.
      - Trace real ownership, fix proven complexity, then measure parser placement: chosen.
    - Chosen Approach:
      - Persist a complete review and derive this executable plan from current source plus review-only measurements.
    - API Notes and Examples:
      ```text
      1 MiB immutable Text index rebuild: 18.0-25.8 ms per edit on reference host
      Current hard frame target: 16 ms
      ```
    - Files to Create/Edit:
      - `docs/development/editor-performance-review-2026-08-26.md`: Completed review and target architecture.
      - `plans/099-Clay-Editor-Performance-Overhaul.md`: Executable remediation plan.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
      - `.agents/skills/project-patterns/references/mode-primitive-first.md`
  - Test Cases to Write:
    - Review-only Vitest measurement: Repeated immutable 1 MiB edits rebuild the current index above the frame budget; temporary test removed after recording results.
  - Completion Evidence:
    - Review written to `docs/development/editor-performance-review-2026-08-26.md`.
    - Independent Antigravity read-only audit confirmed the position-cache miss, bridge coalescing, and parse fan-out findings.
    - Existing focused frontend performance tests pass but were shown not to execute the real Clay edit path.

- [x] Add correlated editor performance instrumentation and establish production baselines
  - Acceptance Criteria:
    - Functional: One content-free numeric trace ID correlates browser input/viewport, CodeMirror update, Tauri enqueue/delivery, server receive/ack, syntax queue/start/end, patch delivery/application, and a paint-adjacent completion marker.
    - Performance: Instrumentation is disabled by default, bounded when enabled, adds no document/path text, and records p50/p95/max plus counts for open, ready, typing, scroll, syntax freshness, React commits, compartment reconfigurations, and long tasks.
    - Code Quality: One trace schema and one recorder adapter replace ad hoc timestamps; existing Rust `PerfRecorder` remains source for server metrics.
    - Security: Traces contain numeric IDs, versions, byte counts, durations, and sanitized feature names only; no source text, package code, paths, credentials, clipboard, or query content.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `.agents/skills/impeccable/reference/optimize.md`
      - `src/perf/metrics.rs`
      - `docs/development/performance.md`
      - `docs/development/editor-performance-review-2026-08-26.md`
      - CodeMirror system guide: transaction and view-update phases.
    - Options Considered:
      - Browser wall-clock tests only: do not locate server/bridge/parser delay.
      - Always-on telemetry: unnecessary privacy/runtime cost.
      - Opt-in bounded correlated local traces: chosen.
    - Chosen Approach:
      - Extend `CLAY_PERF_PROFILE` through the launcher, expose a bounded in-memory frontend recorder, and keep Rust `PerfRecorder` as the server-side source. Enabled typing reuses the existing transaction ID as its trace ID; viewport requests carry an optional trace ID through parse updates and decoration patches. No trace data or authority enters public package APIs.
    - API Notes and Examples:
      ```ts
      const traceId = editorPerformance.trace();
      editorPerformance.mark("browser.input", traceId, { documentId, version });
      editorPerformance.frame(traceId, { documentId, version });
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/performance.ts`, `frontend/src/editor/performance.test.ts`: Schema-v1 bounded recorder, percentiles, long-task observer, and source-free snapshots.
      - `frontend/src/editor/create-editor.ts`, `frontend/src/editor/ClayEditor.tsx`: Browser-input, CodeMirror-update, typing, compartment, React-commit, and paint-adjacent markers.
      - `frontend/src/editor/extensions/controller.ts`, `frontend/src/editor/extensions/types.ts`: Viewport trace creation, enqueue, patch delivery/application, syntax freshness, and trace DTO field.
      - `frontend/src/editor/sync/messages.ts`, `frontend/src/editor/sync/session.ts`: Trace-bearing viewport payloads, open/ready spans, and edit trace/transaction correlation.
      - `frontend/src/app/use-clay-session.ts`, `frontend/src/bridge/types.ts`: Profile activation and developer snapshot hook.
      - `src/protocol/mod.rs`, `src/protocol/parse.rs`, `src/protocol/decorations.rs`, `src/protocol/codec.rs`: Protocol v28 trace fields and round-trip coverage.
      - `src/perf/metrics.rs`: Shared stage names, trace metadata, bounded drop accounting, percentiles, and summaries.
      - `src/client/mod.rs`, `src-tauri/src/bridge/session.rs`, `src-tauri/src/bridge/forwarder.rs`, `src-tauri/src/bridge/dto.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`, `src/launch.rs`: Bridge enqueue/delivery instrumentation, profile propagation, bootstrap flag, and snapshot command.
      - `src/server/connection/mod.rs`, `src/server/connection/documents.rs`, `src/server/parse_coordinator.rs`, `src/server/syntax.rs`: Server receive/ack, patch delivery, syntax queue/start/end, and trace propagation.
      - `src/server/js_runtime/validation.rs`, `src/server/ops/decorations.rs`, `src/server/decorations.rs`, `src/server/mod.rs`: Source-compatible trace defaults in parser/decorations fixtures.
      - `docs/development/performance.md`, `docs/wiki/modules/performance-fixtures.md`, `docs/wiki/modules/desktop-typed-bridge.md`, `docs/wiki/modules/protocol-codec.md`, `docs/wiki/index.md`: Schema, privacy boundary, commands, and baselines.
      - `test-plan/01-launch-and-connection.md`, `test-plan/11-performance.md`: Current protocol-v28 references.
    - References:
      - `docs/development/editor-performance-review-2026-08-26.md`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - Disabled recorder test: zero retained frontend/server events and no trace payload work.
    - Capacity test: bounded ring drops excess and records drop count.
    - Source-safety test: serialized trace contains no fixture text/path/secret markers.
    - Correlation test: one synthetic viewport trace contains every expected stage in monotonic order.
  - Completion Evidence:
    - Frontend Vitest: 25 files / 115 tests passed, including four `PerformanceRecorder` tests and viewport controller tests; typecheck, lint, and format checks passed.
    - Rust: `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib perf::metrics`, `cargo test --lib server::parse_coordinator`, and `cargo test --test protocol` passed (7, 3, and 193 focused tests respectively); `cargo test --all-targets --no-fail-fast` passed (1137 library tests, 30 presentation, 193 protocol, 70 runtime, 130 security).
    - Desktop bridge: `cargo test -p clay-desktop --all-targets --no-fail-fast` passed (28 library, 2 adoption, 1 bridge-session, 4 config-security, and 8 DTO tests).
    - Production frontend build and bundle budget passed: shell gzip 163.4 kB / 180 kB; total gzip 347.0 kB / 400 kB.
    - Protocol v28 trace round-trip and server trace metadata tests passed. `--profile-perf` now reaches the desktop and supervised server; disabled mode retains no trace buffer.
    - Reference-host baseline captured with profiling disabled using 10 samples, 1 s warm-up, and 2 s measurement: protocol hello 106.41 ns median, 16-byte edit 126.24 ns, 1 KiB edit 254.13 ns, one-edit server acknowledgement 249.01 µs, and 128-edit acknowledgement 263.85 µs. Targets and GUI trace capture procedure are recorded in `docs/development/performance.md`.
    - Real GUI launch/screenshot succeeded, but input automation was unavailable because `ydotool` is not installed; no interactive WebKit timing claim was made. Minimum-device promotion remains deferred to the documented three-run rule.

- [x] Obtain approval and record the syntax-session / atomic-viewport-patch decision
  - Acceptance Criteria:
    - Functional: User explicitly approves or rejects the exact architecture: one per-document latest-wins server syntax session, bounded blocking executor, explicit viewport request IDs, and atomic viewport patches; client-local parsing remains deferred behind measurement.
    - Performance: Decision records target metrics and the condition that would reopen parser placement.
    - Code Quality: Alternatives include optimized current server path, CodeMirror/Lezer bundled-language path, and frontend worker Tree-sitter path.
    - Security: Decision records server/package authority, artifact provenance, and no new webview parser/package execution authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-decision-log/SKILL.md`
      - `.agents/skills/project-patterns/SKILL.md`
      - `docs/development/editor-performance-review-2026-08-26.md`
      - `docs/reference/primitives/syntax-vocabulary.md`
      - `docs/reference/packages/creating-packages.md`
      - `decision-logs/2026-07-09-0352-tiered-tree-sitter-themable-syntax-vocabulary-theme-registry-and-opt-in-lsp.md`
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
      - Context7 `/websites/codemirror_net`: language parsers, `ParseContext`, `syntaxHighlighting`, decorations, state fields, and compartment reconfiguration.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
    - Options Considered:
      - Implement major protocol/scheduler changes without approval: rejected.
      - Authoritative CodeMirror/Lezer parsing: theme styling remains possible, but equivalent arbitrary package syntax behavior requires client package execution or a narrower inert adapter.
      - Frontend worker Tree-sitter: preserves grammar family but adds artifact trust, worker authority, duplicate trees, and parity work.
      - Approved server syntax sessions plus atomic patches: chosen.
    - Chosen Approach:
      - Keep package-selected syntax behavior server-side, CodeMirror as local text/render projection, and client-local parsing behind a separate measured decision gate.
    - API Notes and Examples:
      ```text
      Approved: server-side per-document syntax session + atomic viewport patch.
      Theme data remains inert and client-resolved; package parser behavior remains server-owned.
      ```
    - Files to Create/Edit:
      - `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`: Approved decision and package-flexibility rationale.
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`: Durable scheduling/patch rules.
      - `.agents/skills/project-patterns/references/tauri-react-client.md`: CodeMirror projection and parser-placement gate.
      - `plans/099-Clay-Editor-Performance-Overhaul.md`: Completion evidence and approved references.
    - References:
      - `decision-logs/2026-08-26-1838-server-syntax-sessions-and-atomic-viewport-patches.md`
  - Test Cases to Write:
    - Decision-log structure review: Explicit approval evidence, alternatives, metrics, package/theme flexibility, authority, and reconsideration trigger are present.
  - Completion Evidence:
    - User stated, “Architecture is approved.”
    - Decision log records why local CodeMirror theming is compatible but arbitrary package parser behavior is not equivalent without new client authority.
    - Stable protocol/performance and Tauri/React project patterns were updated.

- [x] Fix single-document ownership, progressive loading, and React subscription isolation
  - Acceptance Criteria:
    - Functional: Production bootstrap installs only workspace pane sessions; routed events reach exactly one owning pane session; same-length reload/resync replaces content correctly; detached/remounted pane restores latest user text and metadata.
    - Performance: One document chunk request stream per pane; one current frontend `Text`; programmatic head/chunk/reload/resync changes add zero history entries; unchanged read-only state causes zero compartment reconfigurations; pending/ack changes do not rerender whole workspace or schedule persistence.
    - Code Quality: Session owns detached state only when no view exists; when attached, session references `view.state.doc`. Store subscriptions use stable derived values instead of whole metadata objects.
    - Security: Document/tab routing stays exact; removing singleton must not broadcast one tab's text/features into another session.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `frontend/src/app/use-clay-session.ts`
      - `frontend/src/editor/sync/session.ts`
      - `frontend/src/shell/workspace-controller.ts`
      - CodeMirror reference: transaction annotations and history behavior.
    - Options Considered:
      - Keep global singleton as mirror: duplicates all expensive state and has no production consumer.
      - Keep separate session `Text` beside live view: doubles current text and drifts after edits.
      - One pane session, one current `Text`, detached snapshot only: chosen.
    - Chosen Approach:
      - Remove global singleton from production subscription/bootstrap. Add explicit no-history programmatic transaction helper. Split document metadata into fast editor status and persistence-relevant selectors.
    - API Notes and Examples:
      ```ts
      view.dispatch({
        changes,
        annotations: [
          clayOrigin.of("programmatic"),
          Transaction.addToHistory.of(false),
        ],
      });
      snapshot = view.state.doc;
      ```
    - Files to Create/Edit:
      - `frontend/src/app/use-clay-session.ts`: Remove global document-session install/event routing.
      - `frontend/src/editor/session-singleton.ts`: Delete or make test-only if no production import remains.
      - `frontend/src/editor/sync/session.ts`: Single `Text` owner, content-generation equality, no-history load/replace, latest state on edits.
      - `frontend/src/editor/transactions.ts`: Shared programmatic no-history annotation/spec helper.
      - `frontend/src/editor/ClayEditor.tsx`: Derived read-only subscription/effect.
      - `frontend/src/state/document-store.ts`: Selector-friendly stable metadata projections.
      - `frontend/src/shell/workspace-controller.ts`: Persist/notify only relevant changes.
    - References:
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Bootstrap request-count test: 50 MiB head creates one chunk request per offset, not two.
    - History test: progressive load/reload/resync cannot be undone into partial chunks.
    - Same-length reload test: changed equal-length text is installed.
    - Remount test: latest user edit survives detach/attach without full copy.
    - Render-count test: one user edit and ack produce no workspace pane-tree rerender and no read-only reconfigure.
    - Multi-tab routing test: routed text/features reach only owning client/document.
  - Completion Evidence:
    - Ownership: `frontend/src/editor/sync/session.ts` now keeps exactly one current `Text` per session — `view.state.doc` while attached, a `detachedDoc` snapshot only while detached; `session-singleton.ts` is deleted and `use-clay-session.ts` routes every document/tab envelope through the workspace controller to the owning pane session (no app-wide mirror).
    - Correctness: `installAuthoritative` compares by content (`Text.eq`), so same-length reload/resync with changed text installs; all server-authored installs (head, chunk append, resync, reload, close) dispatch via the shared `programmaticAnnotations()` helper (`clayOrigin "programmatic"` + `Transaction.addToHistory.of(false)`), so progressive loads can never be undone into partial chunks.
    - React isolation: `ClayEditor` reconfigures the read-only compartment only when the derived boolean flips (tracked via `readOnlyProjection`-derived `editable` + `lastReadOnly` ref); `workspace-controller.ts` notifies the shell and schedules persistence only on document identity/path/dirty transitions (`persistenceKeyProjection`), so per-keystroke acks stay pane-local.
    - Tests: 7 new frontend tests — 50 MiB-head single-request-per-offset with duplicate-chunk dedupe, same-length reload install (attached and detached), no-history chunk install (`undo` returns false and text is intact), detach/remount latest-user-text restore, shell-notify/persist gating across edit + ack, and read-only reconfigure count flat across unrelated metadata updates. Frontend Vitest: 25 files / 122 tests passed; typecheck, lint, and format checks passed; production build and bundle budget passed (shell gzip 163.6 kB / 180 kB; total gzip 347.2 kB / 400 kB).
    - Security: routed-event isolation is unchanged and covered by the existing `keeps split trees and documents isolated per tab` test; removing the singleton removes the duplicate broadcast path rather than adding one.
    - Wiki updated: `docs/wiki/modules/react-codemirror-editor.md` (single `Text` owner, no-history installs, subscription-isolation invariants) and `docs/wiki/modules/react-client-bridge.md` (singleton deletion, workspace-controller as sole envelope router).

- [x] Replace immutable-document position cache with an incremental CodeMirror position primitive
  - Acceptance Criteria:
    - Functional: UTF-16↔UTF-8 conversions match Rust golden vectors for Unicode, emoji, combining marks, CRLF, many-line edits, newline insertion/deletion, and edits at document boundaries.
    - Performance: Initial index build is one bounded O(document) install pass; ordinary edit updates only touched lines/segments; conversion is O(log segments + touched-line scan); repeated 1 MiB typing performs no full-document scan and stays inside 8 ms p95 on reference host.
    - Code Quality: One CodeMirror `StateField`/persistent index serves edits, viewport requests, decorations, diagnostics, folds, completion, intelligence, and selections. WeakMap line-text cache is deleted.
    - Security: Invalid/mid-scalar positions snap or reject consistently with Rust; arithmetic is overflow-checked and never creates non-UTF-8 protocol offsets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - Context7 `/websites/codemirror_net`: `StateField`, transactions, `ChangeSet.mapPos`, `Text.line`, `Text.lineAt`.
      - Local `@codemirror/state 6.7.1` types/source for `ChangeSet.iterChangedRanges`, `StateField`, and persistent `Text`.
      - `src/editor/position_map.rs`
    - Options Considered:
      - WeakMap per immutable `Text`: current O(document) miss after each edit.
      - Flat arrays/Fenwick tree: simple for no-newline edits, but line splice near top can remain O(lines).
      - Small persistent order-statistic segment tree updated from changed ranges: chosen unless benchmark proves a simpler chunked-prefix table sufficient.
      - Change protocol to line/UTF-16 everywhere: broad provider/protocol rewrite not needed to fix current issue.
    - Chosen Approach:
      - Implement generic `BytePositionIndex` as editor state, with leaf segments carrying UTF-16/UTF-8/newline totals. Rebuild only changed segments and rebalance under deterministic bounds.
    - API Notes and Examples:
      ```ts
      const byteIndexField = StateField.define<BytePositionIndex>({
        create: (state) => BytePositionIndex.from(state.doc),
        update: (index, transaction) =>
          index.apply(transaction.changes, transaction.state.doc),
      });
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/position-index.ts`: Persistent incremental byte-position index.
      - `frontend/src/editor/position-map.ts`: Keep pure golden-reference conversions; remove WeakMap document index.
      - `frontend/src/editor/create-editor.ts`: Install state field.
      - `frontend/src/editor/sync/operations.ts`: Read index from transaction start state.
      - `frontend/src/editor/extensions/{controller,decorations,diagnostics,folding,completion,intelligence}.ts`: Use shared state field.
      - `frontend/src/editor/position-map.test.ts`: Property/golden/incremental tests.
      - `frontend/src/editor/extensions/performance.test.ts`: Real Clay hot-path operation-count and timing tests.
    - References:
      - `docs/development/editor-performance-review-2026-08-26.md#p0-1-position-index-is-odocument-per-edit-and-retains-duplicated-line-text`
  - Test Cases to Write:
    - Differential property test: Random edit sequences compare incremental index with pure reference conversion after every edit.
    - Work-count test: One-character edit in 50 MiB many-short-lines fixture visits bounded segments independent of document size.
    - Memory test: 256 history states do not retain 256 copied line-string tables.
    - Long-line test: Explicit ceiling/measurement for 1 MiB single line; document remains editable and conversion cost is reported.
  - Completion Evidence:
    - Structure: `frontend/src/editor/position-index.ts` — one persistent order-statistic treap over 64-line chunks (`Uint32Array` UTF-16/UTF-8 widths + phantom-newline weights, deterministic xorshift priorities, divide-and-conquer Cartesian build, treap split/join with path-copying). The WeakMap document index and WeakMap line-text cache are deleted from `position-map.ts`; leaves store numbers only, so history states share structure and can never retain duplicated line-string tables.
    - One field: `bytePositionField` StateField is installed first by `create-editor.ts`; edits, viewport requests, decorations, diagnostics, folds, completion, intelligence, and selections read it via `positionIndex(state)`; edit emission carries the transaction start state's index through `onUserChanges` → `emitUserChanges` → `changesToOperations`, so the keystroke path performs zero rebuilds. The decoration field keeps a private updated copy via the same pure `updatePositionIndex` (CodeMirror field updates cannot safely read other fields' new values).
    - Incremental updates: `updatePositionIndex` maps each `iterChangedRanges` range to its whole-line span (prefix + inserted + suffix region) and replaces only those lines; later ranges shift by accumulated line deltas. Correctness: 8×25-edit differential property tests over multi-chunk (150-line) documents with emoji/combining marks/CRLF compare the incremental index against both the linear Rust-mirrored reference and a fresh rebuild after every edit.
    - Performance: one-character edit adds ≤ 200 tree nodes with identical work at 2 k and 200 k lines (independent of document size); 256 edits on a 20 k-line document grow the index by ≤ 256×250 nodes (structure sharing, no per-state copies); repeated 1 MiB typing on the real `createEditor` path stays inside 8 ms p95 with the shared index asserted present on every edit; 1 MiB single-line conversion cost is reported with an explicit advisory ceiling (O(line) intra-scan is unchanged from the previous index and marked as the future line-segment-split baseline).
    - Security: snap-down semantics (mid-surrogate/mid-code-unit → scalar start) are byte-identical to `src/editor/position_map.rs` golden vectors (`POSITION_MAP_VECTORS`, mirrored in the `clay-desktop` bridge test); offsets clamp to document ends; all weights are `Uint32` and capped by document length.
    - Validation: frontend Vitest 25 files / 127 tests passed (10 position tests incl. 6 new); typecheck, lint, format, production build and bundle budget passed (shell gzip 163.6 kB / 180 kB; total 347.2 kB / 400 kB); Rust `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --test protocol` (193) passed.
    - Wiki updated: `docs/wiki/modules/react-codemirror-editor.md` (shared incremental field, numeric-only weights invariant, new tests).

- [x] Implement atomic, viewport-bounded frontend decorations, diagnostics, and folds
  - Acceptance Criteria:
    - Functional: Complete covered ranges replace exact prior authority; empty patches clear only covered ranges; optimistic transaction mapping preserves unaffected spans; links/inlays/diagnostics/folds remain correct across edits and scroll.
    - Performance: One server patch causes one CodeMirror transaction and one state clone/update; retained render data is visible + bounded overscan; no full retained reprojection, inline style-string rebuild per span, O(N²) diagnostic suppression, or linear fold scan per visible line.
    - Code Quality: CodeMirror state fields are sole render-data owners; `EditorProjection` keeps request orchestration only. Token type maps to predeclared CSS class/facet once.
    - Security: Render data remains inert, validated, token-only, bounded, and cannot inject CSS, HTML, callbacks, URLs, or Tauri calls.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - CodeMirror guide/reference: decorations, `RangeSet.map`, `RangeSet.update`, view plugins, visible ranges.
      - `docs/wiki/modules/decoration-transport.md`
      - `docs/wiki/modules/folding-ranges.md`
      - `docs/wiki/modules/range-diagnostics.md`
    - Options Considered:
      - Preserve one effect per 128-byte set: excessive map/effect/object churn.
      - Flatten entire patch into one full decoration replacement: still rescans retained state.
      - Incremental covered-range update over bounded `RangeSet`: chosen.
    - Chosen Approach:
      - Add one generic patch effect with covered ranges and feature members. Map retained ranges through local edits, filter only covered/evicted ranges, add new ranges, and prune outside latest viewport guard.
    - API Notes and Examples:
      ```ts
      renderField.update(previous, transaction) {
        const mapped = previous.map(transaction.changes);
        return applyViewportPatch(mapped, transaction.effects, transaction.state.field(byteIndexField));
      }
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/extensions/render-patch.ts`: Shared atomic patch state and covered-range update.
      - `frontend/src/editor/extensions/decorations.ts`: Incremental marks/links/inlays.
      - `frontend/src/editor/extensions/diagnostics.ts`: Sorted interval merge/suppression and changed-range lint updates.
      - `frontend/src/editor/extensions/folding.ts`: Sorted fold index and binary lookup.
      - `frontend/src/editor/extensions/controller.ts`: One patch dispatch; remove duplicate caches and heuristic replay.
      - `frontend/src/editor/editor.module.css`: Closed token-class rules only; no new raw package styles.
      - `frontend/src/editor/extensions/extensions.test.ts`: Exact replacement/edit mapping tests.
    - References:
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - `.agents/skills/project-patterns/references/tauri-react-client.md`
  - Test Cases to Write:
    - 100 consecutive viewport patches: retained ranges/bytes stay bounded and update work does not grow with scroll history.
    - Dense 10,000-span patch: one effect/transaction; exact visual token classes and range geometry.
    - Empty patch: clears exact covered range only.
    - Diagnostics overlap sweep: matches reference nested algorithm without quadratic comparisons.
    - Fold lookup: logarithmic candidate lookup over 10,000 sorted ranges.
    - Local edit continuity: unaffected ranges map; covered current authority replaces provisional state.
  - Completion Evidence:
    - Structure: `frontend/src/editor/extensions/render-patch.ts` defines one generic `applyRenderPatch` StateEffect carrying already-projected UTF-16 items (mark/inlay/link/diagnostic/fold), the owning `authority` string, and the covered viewport range. Each patch replaces exactly the same-authority items intersecting its covered range (`replaceCovered` — empty patches clear only that slice), prunes retained items outside covered ± max(4,096, covered) positions (`pruneOutside`), and local edits map items copy-on-write (`mapItems` — untouched items keep identity, so history states share structure; collapsed marks/folds drop).
    - Decoration field: holds projected marks, inlay widgets, and link items; spans are converted once at patch-construction time via the shared incremental index (`decorationPatch`), never re-projected — the old full retained reprojection per patch is gone, along with the field's private index duplicate. Inlay visibility toggles rebuild the decoration set from retained items; links now map through edits (previously stale). Token classes come from one closed `TOKEN_CLASSES` table mapping `TokenType` to predeclared `cm-clay-t-*` rules in `editor.module.css` (plus `cm-clay-m-*` modifier and `cm-clay-f-*` font-role classes) — the per-span inline style string is deleted, and unknown wire token types fall back to an inert class, so no server string can reach CSS.
    - Diagnostics: moved from the imperative `DiagnosticProjection` cache into a state field with covered-range replacement and edit mapping; suppression merges suppressor intervals and binary-searches per span (the previous filter was O(N²)); the lint extension is synced with `setDiagnostics` inside the same transaction (lint maps its own list through edits, both stay consistent).
    - Folds: sorted field with whole-set per-package replacement; the fold service binary-searches the first range at/after the line start and scans only ranges starting on that line (previously every set and range was scanned per visible line).
    - Controller: `EditorProjection` keeps request orchestration only — the three duplicate feature caches and attach-time heuristic replay are deleted; `attach()` simply re-requests the viewport; `clear()` dispatches one reset patch + lint clear in one transaction; each envelope still batches into exactly one dispatch.
    - Dense-patch projection cost: `utf8ToUtf16Batch` converts a whole span list with one resumable per-line cursor (sorted offsets), replacing the N×O(line) per-span scan that made a 10,000-span single-line patch take ~880 ms — now ~25 ms including `Decoration.set`; per-offset results are property-tested identical to the single-shot converter.
    - Tests (`extensions/render-patch.test.ts` + `position-map.test.ts` batch vectors): exact covered-range replacement with sibling/authority isolation; empty-patch scoped clearing; edit mapping of unaffected spans; 100 consecutive viewport patches keep retained marks ≤ guard bound; dense 10,001-span two-effect patch = one transaction, <500 ms state update, correct classes/geometry; suppression sweep matches the reference nested algorithm across 50 random 60-item rounds (found and fixed an empty-span boundary bug); lint sync in one transaction; 10,000-range fold lookup <250 ms with correct line-1 geometry.
    - Validation: frontend Vitest 26 files / 135 tests passed; typecheck, lint, format, production build and bundle budget passed (shell gzip 164.7 kB / 180 kB; total gzip 350.0 kB / 400 kB); Rust `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --test protocol` (193), and `cargo test -p clay-desktop` (43) passed.
    - Wiki updated: `docs/wiki/modules/react-codemirror-editor.md` (render-patch architecture, closed token-class table, bounded overscan), `decoration-transport.md` (client atomic patch application), `range-diagnostics.md` and `folding-ranges.md` (React client fields).

- [x] Replace viewport heuristics and bridge member coalescing with an explicit atomic patch protocol
  - Acceptance Criteria:
    - Functional: Client sends versioned monotonic viewport request ID; server returns one complete success/empty/rejected patch for that request; latest compatible request wins; sibling ranges/packages/features cannot overwrite one another in Tauri.
    - Performance: Normal viewport response uses one bounded protocol envelope rather than dozens of frontend effects; oversized dense output splits into ordered members within one patch identity; Tauri coalesces obsolete whole patches only.
    - Code Quality: Parse context range is separate from authoritative output coverage. Protocol v28 shapes are renderer-neutral and generic across syntax, semantic, diagnostics, and folds where included.
    - Security: Server validates client/document/version/range/request ID and clamps viewport/output sizes before allocation; Tauri stamps identity; packages cannot forge request completion or raw patch data.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `src/protocol/decorations.rs`, `src/protocol/parse.rs`, `src/protocol/mod.rs`
      - `src-tauri/src/bridge/forwarder.rs`
      - `.agents/skills/project-patterns/references/protocol-and-performance.md`
      - Local rkyv 0.8.17 rustdoc/source for bounded archive round trips.
    - Options Considered:
      - Add viewport bounds to current coalesce key only: required hotfix but leaves completion heuristic and N-member frontend shape.
      - Disable all coalescing: preserves correctness but allows stale queue growth.
      - Atomic request-scoped patch with latest-whole-patch coalescing: chosen.
    - Chosen Approach:
      - Add `ViewportRenderRequest` and `ViewportRenderPatch`/rejection. Patch carries request ID, document/version, covered ranges, ordered bounded members, and completion status. Remove 400 ms timer as normal control flow.
    - API Notes and Examples:
      ```rust
      ViewportRenderRequest {
          request_id,
          document_id,
          document_version,
          byte_start,
          byte_end,
      }
      ViewportRenderPatch {
          request_id,
          document_id,
          document_version,
          covered_ranges,
          decorations,
          diagnostics,
          folds,
      }
      ```
    - Files to Create/Edit:
      - `src/protocol/{mod,decorations,parse}.rs`: v28 request/patch/rejection types.
      - `src/protocol/codec.rs`: Envelope/frame bounds and malformed archive tests.
      - `src/client/mod.rs`: Renderer-neutral event projection.
      - `src-tauri/src/bridge/{dto,forwarder,session}.rs`: Typed DTO, identity stamping, whole-patch coalescing.
      - `frontend/src/bridge/types.ts`: Typed patch DTO.
      - `frontend/src/editor/extensions/controller.ts`: Request ID state machine and explicit empty completion.
      - `src/server/connection/{mod,documents}.rs`: Request validation and patch publication.
      - `docs/development/tauri-react-parity-ledger.json`: Protocol family inventory.
    - References:
      - Approved decision log from prior task.
  - Test Cases to Write:
    - Codec/serde round trips for success, empty, rejection, and split member patch.
    - Tauri saturation test: 24 sibling windows remain one complete latest patch; obsolete request patches coalesce.
    - Cross-package test: syntax and semantic patches cannot overwrite each other.
    - Frontend ordering test: stale ID/version drops; explicit empty response immediately frees latest request.
    - Oversized viewport/member rejection before allocation.
  - Completion Evidence:
    - Protocol (v29, `PROTOCOL_VERSION = 29`): `ClientMessage::ViewportRenderRequest` replaces `DecorationViewportRequest` (adds monotonic `request_id: ViewportRequestId`); `ServerMessage::ViewportRenderPatch` (`src/protocol/parse.rs`) carries request id, document/version, `ViewportRenderStatus` (complete/empty/rejected + bounded reason), `covered_ranges`, ordered `decorations`/`diagnostics`/`folds` member vectors, and the optional trace id. Shapes are renderer-neutral rkyv+serde types generic across syntax, semantic, diagnostics, and folds.
    - Server: `handle_viewport_render_request` validates client identity (pre-dispatch boundary), document access, version, and range; rejects invalid ranges and stale versions with one bounded rejection patch, answers unhandled documents with an explicit empty patch, and clamps `byte_end` to the document's total bytes before allocating parse windows. The connection loop aggregates request-scoped updates (`IncrementalParseUpdate.request_id` + `client_id`, stamped by the coordinator) into a per-(document, request) `PendingViewportPatch` counter; every terminal coordinator path of a request-scoped task (failure, stale generation, stale version, validation error) publishes an empty completion update so the counter always reaches zero; a newer request for a document supersedes its pending entry; `finalize_viewport_covered_ranges` derives authoritative output coverage from the members' own viewports — the parse-context range (windows, possibly wider) is never claimed. Edit-driven updates keep the per-update `DecorationBatch`/`DecorationSet`/`DiagnosticSet`/`FoldingRangeSet` frames.
    - Tauri bridge: the forwarder's latest-wins lane now coalesces obsolete whole `ViewportRenderPatch` values per document only; the per-member `DecorationSet`/`DecorationBatch`/`FoldingRangeSet` coalesce keys are deleted (they travel the strict-FIFO live lane), so sibling ranges/packages/features can no longer overwrite one another (the old `batch|{document}` key collapsed across packages). Session identity stamping forwards `request_id`; the patch DTO round-trips through the typed bridge.
    - Frontend: `viewportRenderRequestPayload` carries the request id; `EditorProjection` runs a request-id state machine (monotonic `nextViewportRequestId`, stale patch ids dropped on arrival); the atomic patch reply — not a timer — frees the inflight slot (the 400 ms safety valve is deleted); empty/rejected patches free immediately; member effects still apply in one transaction; edit-driven member frames no longer pace the viewport pipe.
    - Tests: codec round trips for complete (split members), empty, and rejected patches (`protocol_round_trips_viewport_render_patches`); connection test `viewport_render_requests_answer_one_patch_per_request_id` (stale-version rejection, invalid-range rejection before allocation, valid oversized request clamped to document bytes → one complete ordered patch, zero per-member frames); forwarder tests (obsolete whole patch coalesces, 24 mixed syntax/semantic sibling members stay one complete patch, distinct documents never coalesce, live-lane FIFO); frontend tests (stale patch id drops, newest patch members apply, explicit empty/rejected completions free the pipe immediately); updated protocol version pins (agent/editor-intelligence/window-management = 29) and the Tauri/React parity ledger family inventory.
    - Validation: Rust `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, lib tests 1139 passed, `cargo test --test protocol` 193 passed, `cargo test -p clay-desktop` 33 passed; frontend Vitest 26 files / 137 tests, typecheck, lint, format, production build and bundle budget passed (shell gzip 164.7 kB / 180 kB; total gzip 350.1 kB / 400 kB).
    - Docs: parity ledger protocol family inventory updated; wiki pages updated (protocol-codec, react-codemirror-editor, decoration-transport, parse-coordinator, desktop-typed-bridge, index) and `docs/development/performance.md` pacing section rewritten for the atomic protocol.

- [x] Move syntax and mode activation off connection/Tokio hot paths into bounded per-document sessions
  - Acceptance Criteria:
    - Functional: Open/edit/viewport enqueue advisory work and return; chunk requests and subsequent edits are served while mode/syntax/analysis runs; current-version patches preserve grammar behavior and exact package provenance.
    - Performance: At most one active syntax job per document/grammar; native Tree-sitter runs on bounded blocking executor; newer versions/viewports coalesce latest-wins; parser work for one document/language cannot block unrelated connections or same-language documents.
    - Code Quality: Per-document syntax session owns parser/tree/window cache and cancellation state. Shared immutable language/query definitions remain registry-owned. Connection module contains authority dispatch, not parser orchestration.
    - Security: Session creation uses validated current runtime generation, grammar contribution, document access, and package provenance; cancellation/reload/close/revoke removes session and output state.
  - Approach:
    - Documentation Reviewed:
      - `src/server/connection/documents.rs`
      - `src/server/parse_coordinator.rs`
      - `src/server/syntax.rs`
      - `src/server/document.rs`
      - `.agents/skills/rust-async-patterns/SKILL.md`
      - `.agents/skills/rust-best-practices/references/chapter_03.md`
      - `.agents/skills/rust-best-practices/references/chapter_09.md`
      - Exact local `tokio 1.52.2`, `tree-sitter 0.25.10`, and `crop 0.4.3` APIs/source.
    - Options Considered:
      - Add semaphore around current `tokio::spawn`: limits fan-out but still blocks Tokio and retains grammar-global parser mutex.
      - `spawn_blocking` per current task: removes Tokio block but not per-document latest-wins/tree ownership.
      - Bounded per-document syntax sessions over blocking executor: chosen.
      - Client parser replacement: deferred decision gate.
    - Chosen Approach:
      - Add generic `SyntaxSession` keyed by document/grammar/generation. Mailbox holds latest version plus latest viewport. Worker snapshots required rope range only when selected to run, then performs parser/query work on blocking pool and publishes one atomic patch.
      - Runtime generation builds a validated static mode/grammar registry. Measure generated V8 open activation; replace ordinary per-open generated module evaluation with registry classification when behavior parity tests pass.
    - API Notes and Examples:
      ```rust
      enum SyntaxJob {
          DocumentChanged { version, edit },
          ViewportChanged { request_id, version, range },
          Close,
      }
      // Mailbox retains latest compatible state; one blocking worker per active session.
      ```
    - Files to Create/Edit:
      - `src/server/syntax_session.rs`: Per-document scheduler/session and bounded blocking execution.
      - `src/server/syntax.rs`: Immutable grammar/query definitions and session-owned parser/tree operations.
      - `src/server/parse_coordinator.rs`: Registration/output validation; remove native task fan-out ownership.
      - `src/server/connection/documents.rs`: Nonblocking enqueue after required response.
      - `src/server/document.rs`: Bounded rope snapshot methods called by selected worker only.
      - `src/server/mod.rs`: Lifecycle wiring, executor limit, close/reload cleanup.
      - `src/server/js_runtime/mod.rs`: Validated mode/grammar registry snapshot and measured activation handoff.
      - `src/perf/budgets.rs`: Syntax executor/session/window-cache bounds.
    - References:
      - Approved decision log from prior task.
      - `.agents/skills/project-patterns/references/authority-boundaries.md`
  - Test Cases to Write:
    - Slow parse test: Chunk request and next edit ack complete while syntax worker is blocked.
    - Tokio starvation test: One CPU-heavy parser job does not delay unrelated timer/connection task.
    - Same-language two-document test: Jobs progress independently within executor limit.
    - Latest-wins test: 100 viewport/version updates publish only latest compatible patch.
    - Close/reload/generation replacement test: Worker cancels, cache clears, stale output cannot publish.
    - Window revisit test: Bounded stable-window cache reuses measured windows and respects byte budget.
    - Mode classification parity test: Static registry matches current package/runtime classification fixtures without per-open generated module evaluation.
  - Completion Evidence:
    - Sessions: `src/server/syntax_session.rs` adds `SessionMailbox` (latest-wins, monotonic job sequences, shared `observed` watermark so a drained job is never mistaken for pending; `close()` drops a never-delivered pending job and terminates the worker after its current job) and `SyntaxExecutor` (shared bounded blocking executor). `ParseCoordinator::schedule_parse_with_windows` now creates one persistent session per (generation, document, grammar) — the window identity rides on the job, so every viewport change coalesces into the same mailbox. First schedule spawns the worker; later schedules enqueue latest-wins; request-scoped schedules are idempotent per request id while edit/viewport schedules always enqueue. `abort_tasks` became `close_sessions`: mailboxes close, pending request jobs complete at close time, workers exit gracefully after their current job (a running job is never aborted mid-parse; its output is discarded at publication with an empty completion for request-scoped jobs). `remove_document`, `cancel_package`, `cancel_generation`, `cancel_older_generations`, and generation-stale handler registration all route through it.
    - Execution: native Tree-sitter handlers gained `ParseHandler::parse_blocking` + `runs_on_blocking_executor`; the session worker runs them via `spawn_blocking` under `SYNTAX_EXECUTOR_MAX_JOBS = 4` permits, so parser CPU never runs on a Tokio worker thread and worst-case concurrent syntax memory is capped. Package JavaScript handlers still await on the runtime worker. Queue-wait/start/end metrics record per job under the job's trace id.
    - Per-document parsers: `TreeSitterSyntaxHandler` lost its grammar-global `Arc<Mutex<Parser>>`; each document's `CachedSyntaxState` owns its parser + latest tree, so same-language documents parse concurrently. The cache is bounded by `SYNTAX_DOCUMENT_TREE_CACHE_ENTRIES = 64`; `parser_cache_id` (unused) was deleted.
    - Mode activation fast path: `classify_open_document` caches each completed V8 open activation per runtime generation, keyed by (generation, path extension/name, shebang line, leading-content probe hash) with `MODE_ACTIVATION_CACHE_ENTRIES = 64`. A repeat open that hits the cache — and whose native grammar is still registered for the generation — republishes the cached behavior manifest from Rust and skips the generated module evaluation; misses (cold registry, third-party modes) keep the V8 path. Measured: generated V8 open activation ≈ 15.6 ms on the dev host; the fast path publishes without V8.
    - Tests: coordinator session tests with a deterministic gate handler (`blocking_syntax_job_does_not_starve_tokio_timer`, `session_mailbox_coalesces_hundred_updates_to_latest` — 100 enqueues publish exactly one latest patch, `same_language_two_documents_progress_independently`, `superseded_and_closed_request_jobs_publish_completions` — superseded running jobs and closed pending jobs each publish exactly one empty request completion); mailbox/executor unit tests; `document_tree_cache_is_bounded_and_windows_respect_byte_budget` (bound + WindowTooLarge); `mode_activation_cache_hit_skips_generated_module_evaluation` (parity of activation identity and behavior manifest, zero V8 evaluations on repeat open, V8 activation measured). Existing semantics preserved: all prior coordinator/connection/viewport/JS-handler suites pass unchanged apart from tighter latest-wins behavior.
    - Compromises: (1) window snapshots are still built at enqueue time from the already-open document (bounded rope slices) rather than inside the selected worker — the CPU-bound parse moved off the runtime, the O(window) slice stays on the connection task; move slicing into the worker if profiling shows it matters. (2) A session worker binds its handler at spawn; a same-generation handler replacement after a session exists does not rebind that session (production replacement happens before first use or under a new generation, which closes sessions). (3) Tree-cache eviction beyond the bound is arbitrary, not LRU.
    - Validation: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, lib tests 1149 passed, protocol 193 passed, clay-desktop 33 passed; wiki page `docs/wiki/modules/syntax-sessions.md` added and indexed; `docs/development/performance.md` records the bounds and measurement.

- [x] Run automated and real-device editor performance matrix and enforce stable invariants
  - Acceptance Criteria:
    - Functional: Open/type/scroll/fold/save/reload/resync pass for 64 KiB, 1 MiB, 10 MiB, and 50 MiB fixtures across plain text, Rust, TypeScript/TSX, JavaScript, and Markdown; one- and four-pane flows work.
    - Performance: Reference and minimum-device measurements meet approved p95 targets; zero >50 ms long tasks during 5-second typing/scroll traces; one active syntax job per document; bounded frontend/server retained bytes.
    - Code Quality: CI blocks deterministic work-count/ownership/cache/queue/history invariants. Machine-variant timings become blocking only after three stable minimum-device runs.
    - Security: Generated fixtures stay under approved temp/target roots; traces contain no user content or absolute paths.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/development/performance.md`
      - `docs/wiki/modules/performance-fixtures.md`
      - `scripts/large-document-smoke.sh`
      - `test-plan/11-performance.md`
    - Options Considered:
      - jsdom timing only: misses WebKit/React/Tauri/server.
      - Reference workstation only: hides minimum-device regressions.
      - Deterministic CI invariants plus designated physical Linux device timings: chosen.
    - Chosen Approach:
      - Add a real Tauri/WebKit scripted trace command using generated fixtures and a separate browser fixture for fast triage. Record reference host and physical minimum-device tables.
    - API Notes and Examples:
      ```bash
      CLAY_PERF_PROFILE=1 scripts/editor-performance-smoke.sh \
        --sizes 64KiB,1MiB,10MiB,50MiB \
        --kinds mixed-unicode,many-short-lines,long-lines,newline-heavy
      ```
    - Files to Create/Edit:
      - `scripts/editor-performance-smoke.sh`: Real desktop fixture/trace matrix.
      - `frontend/src/editor/extensions/performance.test.ts`: Real Clay path and deterministic work counts.
      - `tests/editor_performance.rs` or consolidated suite module: Server queue/concurrency/patch bounds.
      - `tests/large_document.rs`: Retain transfer/save tests and add trace IDs where appropriate.
      - `docs/development/performance.md`: Before/after tables and designated device details.
      - `test-plan/11-performance.md`: Manual device steps and ceilings.
    - References:
      - `docs/development/editor-performance-review-2026-08-26.md#performance-targets-and-device-strategy`
  - Test Cases to Write:
    - 1 MiB repeated edit test executes Clay listener/index/controller, not bare CodeMirror.
  - Completion Evidence:
    - CI-blocking automated matrix (`tests/editor_performance.rs`, in the `runtime` suite): one real `IpcServer` per run, driven over the typed protocol for 30 generated-fixture cells — 64 KiB across all four shapes x all six language extensions, 1 MiB across five languages, 10 MiB and 50 MiB plain text. Per cell it asserts: exactly one atomic `ViewportRenderPatch` per viewport request id (no duplicates within a quiet drain), document mode manifest matches the path extension (`markdown.markdown`, `rust.rust`, `typescript.typescript` for both ts/tsx, `javascript.javascript`, `default.text`), exact edit/version accounting (ack advances exactly one version), save/reload/resync round-trips preserving the authoritative text (reload of a saved file keeps the version), and close retirement (`DocumentClosed`; no late patches). Fixtures are generated under approved roots via `clay perf-fixture` only.
    - CI-blocking frontend invariants (`frontend/src/editor/extensions/performance.test.ts`, rewritten): 200 repeated 1 MiB edits on the real `createEditor` path where the shared position field value tracks the current document version exactly and every edit converts through it (no rebuild); constant-size retention across 100 sliding viewport patches (spans per patch retained exactly, strides beyond the guard); 50 MiB document installed as a single authoritative `Text` with no history, surviving detach as the one current snapshot; four-pane patch application linear in visible panes (each pane retains exactly its own patch); software-render functional smoke (text intact after decoration application). Wall-clock timings are no longer asserted — they are machine-variant.
    - Real-device harness (`scripts/editor-performance-smoke.sh`): builds the frontend with `VITE_CLAY_PERF_PROFILE=1`, generates fixtures under `target/perf-fixtures/` (the only roots the generator accepts), copies each under `.txt/.md/.rs/.ts/.tsx/.js` for path-driven classification, and launches a profiled server (`--config-fixture clay-performance-matrix`, preloading markdown/rust/typescript/javascript) plus the desktop. Closing the window writes three source-free reports under `target/perf/editor-performance/<label>/`: browser/CodeMirror snapshot (new dev-only `write_frontend_perf_report` Tauri command, flushed on a 10 s interval to avoid teardown races), desktop bridge summary (`RunEvent::Exit`), server summary (SIGTERM dump) — all via `perf::metrics::write_perf_report` gated on `CLAY_PERF_REPORT_DIR` (label-sanitized, atomic rename). A Python analyzer merges them into `summary.json` (p95 table, long-task count, verdict).
    - Stable invariants enforced: CI runs the two deterministic suites (work counts/ownership/retention/history — never wall clock). The harness always checks bounded retention (<= 4096 events) and fails on missing reports; `--enforce` additionally blocks on zero long tasks > 50 ms. Stage-presence gaps (checklist not driven) are warnings that gate the timing table, not the verdict. Timing p95s become blocking only after three stable designated-device runs, recorded per run under `target/perf/`.
    - Reference-host execution record (2026-08-27): `target/perf/editor-performance/ref-run-2/` verdict pass with all three reports captured (frontend interval flush verified live; desktop/server dumps on close). Interactive type/scroll flows were NOT drivable on that host — no keyboard input backend (ydotool/xdotool/uinput all unavailable; WebKitGTK exposes no internal AT-SPI nodes), so the run's p95 table covers protocol codec and runtime-load stages only, and M2/M3 timing evidence is pending the designated device. Manual device steps recorded in `test-plan/11-performance.md` (M1-M7, added to the parity ledger).
    - Harness bug found and fixed en route: the mode-activation fast path republished cached manifests under the ORIGINAL document's scope, so the new document's layer lookup missed and classification fell back to `default.text` (`mode_activation_cache_hit` parity test still passes; the matrix's 1 MiB markdown cell caught it).
    - Validation: `cargo test --test runtime editor_performance_matrix` (30/30 cells), frontend vitest 139 passed, tsc/eslint/prettier clean, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, protocol 193 passed, clay-desktop suites passed; docs updated (`docs/development/performance.md`, `docs/wiki/modules/performance-fixtures.md`, `test-plan/11-performance.md`, parity ledger).
    - 100-scroll patch retention test remains constant-size.
    - 50 MiB one-session/no-history/one-current-Text invariant.
    - Four-pane aggregate patch/work count remains linear in visible panes.
    - Software-rendering smoke remains functional without lost text/highlights.

- [x] Decide whether a client-local parser spike is still necessary
  - Acceptance Criteria:
    - Functional: Post-overhaul measurements are compared against approved syntax freshness and scroll targets for every first-party mode.
    - Performance: Spike proceeds only when sustained p95 misses remain attributable to server/bridge parser latency rather than frontend application, device noise, or unoptimized queries.
    - Code Quality: If needed, compare CodeMirror/Lezer and frontend worker Tree-sitter in an isolated branch/fixture without changing production authority or package APIs.
    - Security: Any local-parser recommendation includes artifact integrity, worker isolation, third-party grammar policy, server/headless parity, and package provenance; no implementation lands without explicit approval and new decision log.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - Context7 `/websites/codemirror_net`: language packages and incremental parsing.
      - `decision-logs/2026-07-19-0351-low-latency-incremental-syntax-decoration.md`
      - `decision-logs/2026-08-23-0052-tauri-react-client-architecture.md`
    - Options Considered:
      - Always run spike: speculative dependency/architecture work.
      - Never reconsider parser placement: ignores measured architecture ceiling.
      - Metric-gated bounded spike: chosen.
    - Chosen Approach:
      - Treat the Plan 100 grammar rejection and server-authoritative resume decision as the current parser-placement baseline. Revisit this task only after the completed server-session path misses approved metrics and traces identify server/bridge placement as the remaining cause.
    - API Notes and Examples:
      ```text
      Trigger: sustained minimum-device viewport-to-current-syntax p95 above approved target
      after frontend patch apply and server queue/parser stages are individually within budget.
      ```
    - Files to Create/Edit:
      - `code-reviews/<date>-client-parser-spike.md`: Only if trigger fires.
      - No production files unless separately approved.
    - References:
      - `docs/development/editor-performance-review-2026-08-26.md#architecture-options-considered`
      - `docs/development/client-local-parsing-spike-2026-08-26.md`
      - `decision-logs/2026-08-27-0159-resume-server-authoritative-editor-performance.md`
  - Test Cases to Write:
    - Post-overhaul decision check: Compare approved server-session metrics against syntax freshness, package provenance, headless parity, and memory targets before reopening any client parser work.
    - Spike parity matrix, only if the reopen condition triggers: Syntax captures, incremental edits, large-file fallback, package provenance, theme tokens, headless/server behavior, bundle/memory cost.
  - Completion Evidence (decision: NO — do not run a client-local parser spike now):
    - Reopen trigger not fired. The gate (this plan, `decision-logs/2026-08-27-0159-resume-server-authoritative-editor-performance.md`, and the `protocol-and-performance.md` fail-fast checkpoint) requires the completed server-session path to MISS approved metrics with traces attributing the miss to server/bridge parser placement. No sustained p95 miss exists: the automated matrix passes 30/30 cells on deterministic invariants (exactly one atomic patch per request id, exact edit/version accounting, save/reload/resync, close retirement), and every measured latency stage is far inside budget — server document acknowledgement p95 249-264 µs against the 40 ms target (~150x headroom), protocol codec stages sub-millisecond on both ends, warm runtime load 12.7 ms, cached mode activation skips the 15.6 ms V8 evaluation, zero long tasks >= 50 ms in the reference harness run (`target/perf/editor-performance/ref-run-2/summary.json`).
    - Interactive typing/scroll freshness p95 (`editor.paint_adjacent`, `editor.typing`, `editor.syntax_fresh`) is pending the designated-device runs (test-plan M2/M3), not a demonstrated miss. Closing that gap is a matrix run on existing tooling, not a parser spike. If those runs later miss targets, attribution runs through the per-stage trace budgets (frontend patch apply, syntax queue/start/end, bridge delivery) before any placement question reopens.
    - The Plan 100 grammar-freshness rejection stands independently of performance: frozen stock client-local Lezer grammars produce recovery nodes on modern Rust (4) and TypeScript (2), so the client-local Lezer candidate fails its primary function regardless of latency. The only remaining local alternative (frontend-worker Tree-sitter) is a separate architecture that still requires the full artifact-integrity/worker-isolation/grammar-policy/provenance assessment and a new decision log before any implementation.
    - Security posture unchanged: no local-parser recommendation is being made; production parsing authority stays server-side; no production files changed under this task.
    - Re-evaluation condition (unchanged from the chosen approach): sustained minimum-device viewport-to-current-syntax p95 above the approved scroll targets across three stable designated-device runs, with traces showing frontend patch apply and server queue/parser stages individually within budget. Only that finding reopens parser placement, via explicit approval and a new decision log.

- [x] Perform visual screenshot and accessibility review of changed editor states
  - Acceptance Criteria:
    - Functional: Real Linux Tauri build exercises small/large open, progressive loading, ready editor, rapid scroll, pending syntax/plain fallback, diagnostics, folds, error/recovery, one/four panes, narrow/wide layouts, and light/dark/user typography.
    - Performance: Review confirms no visible blank-text wait, stuck highlight overlay, input freeze, or scroll hitch in representative traces; findings reference recorded metrics rather than subjective pass alone.
    - Code Quality: Screenshots, trace summaries, and findings are stored under a named review artifact path; failures are fixed or explicitly unresolved.
    - Security: Artifacts use synthetic fixture content, sanitize host paths, and contain no credentials/user documents.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/clay-ui/SKILL.md`
      - `.agents/skills/clay-ui/references/components.md`
      - `.agents/skills/clay-ui/references/tokens.md`
      - `.agents/skills/impeccable/SKILL.md`
      - `.agents/skills/full-output-enforcement/SKILL.md`
      - `.agents/skills/high-end-visual-design/SKILL.md`
      - `.agents/skills/design-taste-frontend/SKILL.md`
      - `docs/reference/ui-components.md`
      - `docs/wiki/modules/ui-review-harness.md`
      - `docs/development/launch-and-gui-smoke.md`
      - `.agents/skills/project-patterns/references/ui-visual-review.md`
    - Options Considered:
      - Source/tests only: cannot prove WebKit frame behavior or accessible loading semantics.
      - Browser fixture only: misses Tauri/WebKit/bridge/server.
      - Real Tauri review plus fixture fallback: chosen.
    - Chosen Approach:
      - Start computer-use with `get_app_state`, inspect accessibility tree, keyboard/focus, live loading/ready/error statuses, and capture screenshots/traces in one bounded review pass plus one confirmation pass.
    - API Notes and Examples:
      ```text
      code-reviews/screenshots/<date>-plan099-editor-performance/
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/sync/session.ts`: Track each in-flight open path for restore-reply attribution.
      - `frontend/src/shell/workspace-controller.ts`: Route multi-pane open replies and publish shell-visible status snapshot changes without ack churn.
      - `frontend/src/state/document-store.ts`: Stable loading/diagnostic shell-status projection.
      - `frontend/src/shell/workspace-controller.test.ts`: Multi-pane routing and shell-status notification regressions.
      - `scripts/capture-editor-performance-review.sh`: Synthetic diagnostics fixture and real Tauri state capture.
      - `docs/wiki/modules/{react-codemirror-editor,ui-review-harness}.md`: Updated implementation and review flow.
      - `code-reviews/screenshots/<date>-plan099-editor-performance/`: Screenshots, semantic dumps, trace summaries, review log.
    - References:
      - `.agents/skills/clay-ui/SKILL.md` visual/a11y proof rules.
  - Test Cases to Write:
    - Keyboard-only focus reaches editor after ready; loading gate has named status; error remains actionable.
    - Screen reader sees current editable document only, not duplicate hidden session text.
    - Four-pane scroll/focus semantics remain distinct and responsive.
  - Completion Evidence:
    - Final-build real Tauri artifacts are stored under `code-reviews/screenshots/2026-08-28-plan099-editor-performance/`: light, dark four-pane, progressive 50 MiB loading, large typography, diagnostics, binary rejection, and resident-budget rejection states. Each has frontend, desktop, and server reports; staged loading screenshots show immediate head and final analysis-limit status.
    - The dark four-pane screenshot shows independent `notes.md`, `module.ts`, `review.rs`, and `medium.txt` documents with correct syntax projections. Binary and budget screenshots show typed actionable server errors. Real traces retain 0 dropped events; the large-load run records 199 chunk applications, `editor.ready`, and `editor.paint_adjacent` p95 6 ms.
    - `review-log.md` and `accessibility-review.txt` record screenshot findings, AT-SPI evidence, doctor output, synthetic-only/security checks, and explicit unresolved items. Native fixture batch has PASS for default/large typography and explicit UNRESOLVED statuses for loading/error/recovery.
    - Review found and fixed two regressions: multi-pane `documentOpened` routing now matches each session's in-flight path before placeholder document ids; shell loading/diagnostic transitions now publish a fresh external-store snapshot without re-rendering on ack churn. Regressions are covered in `frontend/src/shell/workspace-controller.test.ts`.
    - Remaining limitations: no visible Marksman diagnostic marker was confirmed after two waits; keyboard-only focus/typing/scroll could not run because this Wayland host has no keyboard input backend. Both are recorded as unresolved rather than passed.
    - Validation: frontend typecheck and targeted Vitest passed; final frontend production build passed; final Tauri desktop build passed; all visual screenshots were inspected.

- [x] Create or verify Clay JS APIs for public programmatic surfaces
  - Acceptance Criteria:
    - Functional: Review all changed public Rust functions and confirm viewport patch/syntax scheduling remains internal; no package/user JS API is added unless a real extensibility need exists.
    - Performance: Public API docs retain no promise that encourages full-document decoration publication or synchronous parse work.
    - Code Quality: Internal functions are private/`pub(crate)`; any intentional API follows stable dotted IDs, facade wrappers, inventory, docs, lookup, and generated registry requirements.
    - Security: No raw parser handle, Tauri command, blocking executor, document rope, viewport completion spoofing, or performance trace content is exposed to packages.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-naming.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/reference/clay-js-api/inventory.md`
      - `docs/reference/clay-js-api/schema.md`
      - `docs/reference/primitives/registry.md`
      - `docs/index.md`
      - `src/server/facades.rs`
      - `tests/clay_js_api_inventory.rs`
      - `tests/clay_js_doc_registry.rs`
      - `tests/rust_visibility_api_mapping.rs`
    - Options Considered:
      - Expose syntax scheduler tuning: rejected; server-owned performance/security budgets.
      - Expose raw viewport patch requests to packages: rejected; client/server internal render transport.
      - Expose profile-only report commands through Clay JS: rejected; Tauri bridge is developer-harness plumbing, not server package authority.
      - Verify-only unless implementation introduces genuine public behavior: chosen.
    - Chosen Approach:
      - Inventory every changed Rust public item, classify cross-crate protocol/performance/Tauri bridge exceptions, and keep scheduler, parser, patch, trace, and document state plumbing outside the Clay JS registry.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js_api
      cargo test --test protocol clay_js_doc_registry
      cargo test --test security rust_visibility_api_mapping::
      ```
    - Files to Create/Edit:
      - `plans/099-Clay-Editor-Performance-Overhaul.md`: Record the verification result; no public API files changed.
      - `docs/reference/clay-js-api/api-inventory.toml`: Only if intentional API changes.
      - `docs/reference/clay-js-api/**/*.md`: Only for intentional API changes.
      - `docs/index.md`: Link any intentional API docs.
      - Generated registry artifacts: Update through documented command if docs change.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
      - `docs/wiki/modules/clay-js-doc-registry.md`
  - Test Cases to Write:
    - API inventory/registry/docs guards pass.
    - Source visibility review: New scheduler/patch helpers are not public without facade/docs.
  - Completion Evidence:
    - No new Clay JS/TypeScript API was required. `runtime/js/*`, `src/server/facades.rs`, `docs/reference/clay-js-api/api-inventory.toml`, `docs/index.md`, and `docs/generated/clay-js-api-registry.json` remain unchanged by Plan 099's performance work; existing `parse.serverRegisterParseHandler` remains the package parser registration boundary.
    - Visibility audit covered every changed Rust declaration: `src/server/connection/{documents,mod}.rs`, `src/server/js_runtime/mod.rs`, `src/server/parse_coordinator.rs`, and `src/server/syntax_session.rs` keep viewport, mode-cache, syntax-session, executor, and trace helpers private/`pub(crate)`/`pub(super)`. `ParseHandler::{parse_blocking,runs_on_blocking_executor}` are default scheduler hooks on the existing server trait, not package-selectable APIs; package JavaScript still uses the token-backed parse facade.
    - Public protocol additions (`PerformanceTraceId`, `ViewportRequestId`, `ViewportRenderStatus`, `ViewportRenderPatch::rejected`) are typed root/Tauri transport DTOs, not user/package programmatic surfaces. Public `src/perf/metrics.rs` summaries/report helpers are cross-crate, profile-only observability plumbing consumed by `src-tauri`; `session_perf_snapshot` and `write_frontend_perf_report` are profile-gated Tauri harness commands, not server-side Clay JS facades. None exposes parser handles, document ropes, executors, patch completion, or trace contents to package JavaScript.
    - Naming and registry audit: existing public IDs remain bare reserved-domain IDs; no new core domain, callable export, key binding, custom property, permission, Markdown page, docs-index link, or generated registry entry was introduced.
    - Validation: `cargo test --test protocol clay_js_api` (13), `cargo test --test protocol clay_js_doc_registry` (48), `cargo test --test protocol primitives_docs` (30), `cargo test --test protocol documentation_coverage` (10), `cargo test --test security rust_visibility_api_mapping::` (4), and `cargo run --bin update-doc-registry` all passed; generated registry is current. `git diff --check` and final frontend formatting checks passed.

- [x] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review confirms parser concurrency, caches, overscan, request pacing, trace capacity, and device budgets remain host-owned unless explicit user value is proven.
    - Performance: No configuration option can raise hard memory/frame/concurrency limits into unsafe ranges or force synchronous/full-document syntax.
    - Code Quality: Any user-visible preference is a documented Clay JS API through `~/.config/clay/init.js`; no hidden environment/config key becomes normal product behavior.
    - Security: Configuration grants no new filesystem, network, shell, process, parser artifact, Tauri, package, or AI authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `.agents/skills/project-patterns/references/clay-js-api-boundary.md`
      - `.agents/skills/project-patterns/references/clay-js-api-schema.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `docs/reference/clay-js-api/configuration/set-package-option.md`
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `runtime/js/configuration.js`
      - `runtime/js/configuration.d.ts`
      - `examples/init.js`
      - `docs/development/performance.md`
      - `docs/wiki/modules/configuration-runtime.md`
      - `src/server/configuration.rs`
      - `src/perf/budgets.rs`
      - `tests/clay_js_api_inventory.rs`
      - `tests/clay_js_doc_registry.rs`
      - `tests/performance_budgets.rs`
    - Options Considered:
      - User-tunable low-level performance knobs: rejected; scheduler, memory, parser, viewport, and trace limits are host safety policy.
      - Developer-only opt-in trace environment: retained, bounded, not user configuration.
      - Verify-only configuration task with a canonical-example repair: chosen.
    - Chosen Approach:
      - Keep scheduler/cache/patch budgets compiled and measured, correct the stale canonical `setPackageOption` note, document the Plan 099 closure, and extend existing hidden-key/docs guards without adding a facade or registry entry.
    - API Notes and Examples:
      ```text
      Expected result: existing documented APIs cover user choices; no new init.js option for syntax threads, patch size, cache bytes, trace capacity, device budgets, or chunk pacing.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Record the Plan 099 host-owned configuration review and rejected hidden keys.
      - `examples/init.js`: Mark `setPackageOption` runtime-backed, retain the three unavailable stubs, and show one safe package-option example.
      - `src/server/configuration.rs`: Reject Plan 099 performance/retention/trace option names through the existing closed allowlist test.
      - `tests/clay_js_doc_registry.rs`: Guard canonical-example status and Plan 099 configuration-doc markers.
      - `docs/wiki/modules/configuration-runtime.md`: Document the Plan 099 configuration closure.
      - `docs/generated/clay-js-api-registry.json`: Verify unchanged through the registry update command.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
      - `docs/reference/clay-js-api/configuration.md#plan-099-editor-performance-configuration-review`
      - `docs/development/performance.md#plan-099-correlated-editor-trace-and-production-baseline`
  - Test Cases to Write:
    - Closed configuration surface test rejects internal performance/security budget names.
    - Registry/docs guard records Plan 099 bounds, developer-only profiling, and no new public API.
    - Canonical-example cross-check distinguishes runtime-backed `setPackageOption` from planned stubs.
    - `node --check examples/init.js` and all modular example files pass.
  - Completion Evidence:
    - Surface decision: no new Clay JS configuration API is needed. Existing documented surfaces cover package loading, syntax-tier selection, themes/typography/appearance, key bindings, LSP grants, and package-owned UI defaults. `clay:configuration` remains exactly three runtime-backed exports (`loadConfigurationModule`, `getConfigurationState`, `setPackageOption`) plus three planned/unavailable stubs (`setModePreference`, `setDecorationTheme`, `setParsePolicy`); `docs/reference/clay-js-api/api-inventory.toml`, `runtime/js` facades, `src/server/facades.rs`, `docs/index.md`, and the generated registry require no new entry.
    - Canonical example repaired: `examples/init.js` now documents `setPackageOption` as runtime-backed with its seven supported suffixes and a safe commented Markdown example; only the three unavailable configuration stubs remain under “Planned — NOT callable yet”. The example remains copy-safe and modular through `~/.config/clay/init.js` plus optional local modules.
    - Host-owned Plan 099 controls documented and kept out of configuration: four blocking syntax permits, 64 syntax-tree cache states, 64 mode activations per generation, 30 MiB syntax cache, 256 MiB resident-memory envelope, 256 KiB chunks/768 KiB native context, 4096-position render overscan, and 4096 metadata-only trace events. `CLAY_PERF_PROFILE`, `VITE_CLAY_PERF_PROFILE`, and `--profile-perf` remain developer-only measurement paths.
    - Security: `src/server/configuration.rs::plan060_internal_security_and_performance_controls_are_not_configurable` now rejects Plan 099 parser, viewport, retention, document-memory, device-budget, and trace-capacity names through the existing closed package-option boundary. No configuration route grants filesystem, network, shell, process, parser artifact, package-control, AI, workspace, raw-op, native-widget, or client-side JavaScript authority.
    - Deterministic coverage: `tests/clay_js_doc_registry.rs::plan099_configuration_surface_keeps_editor_performance_controls_host_owned` pins the docs markers, existing public package-option API, and unavailable parse-policy API; canonical-example checks cover the runtime-backed/planned split. `tests/clay_js_api_inventory.rs` continues to pin the exact six configuration exports and security metadata.
    - Validation: `cargo test --test protocol clay_js_api` (13), `cargo test --test protocol clay_js_doc_registry` (49), `cargo test --test protocol performance_budgets` (28), `cargo test --lib server::configuration` (18), `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `cargo run --bin update-doc-registry`, `node --check` on all three canonical example modules, and `git diff --check` passed. Generated registry remains unchanged.
    - Wiki updated: `docs/wiki/modules/configuration-runtime.md` records the Plan 099 closure and links the authoritative configuration reference.

- [x] Execute and update the manual test plan
  - Acceptance Criteria:
    - Functional: Real Linux build executes affected modules: launch/connection, files/workspace, core editing, syntax/textobjects, tabs/splits, and performance. New steps cover progressive ready, one-session routing, typing during delayed syntax, fling/jump scroll, explicit empty patch, and recovery.
    - Performance: Manual record includes reference/minimum device, WebKit version, file/mode/line-shape matrix, p95 metrics, long-task count, memory, and parser queue count.
    - Code Quality: Existing steps are not weakened; known ceilings are recorded explicitly; coverage matrix links deep performance docs.
    - Security: Fixtures/evidence are synthetic and sanitized; no ambient user files or secrets are retained.
  - Approach:
    - Documentation Reviewed:
      - `test-plan/index.md`
      - `test-plan/01-launch-and-connection.md`
      - `test-plan/03-files-and-workspace.md`
      - `test-plan/04-core-editing.md`
      - `test-plan/08-syntax-and-textobjects.md`
      - `test-plan/11-performance.md`
      - `test-plan/13-window-splits.md`
      - `test-plan/14-tabs.md`
      - `docs/development/performance.md`
      - `docs/development/tauri-react-parity-ledger.json`
    - Options Considered:
      - Automated only: misses physical WebKit/device behavior.
      - Unstructured manual notes: not repeatable.
      - Extend numbered modules and record exact environment/results: chosen.
    - Chosen Approach:
      - Execute after automated and visual checks so manual work validates final build.
    - API Notes and Examples:
      ```text
      M1–M7: 1/10/50 MiB fixture matrix; open, type, scroll, fold, save/reload, four-pane, and recovery checks.
      F53/S33/S34: one-session progressive loading, request-id viewport continuity, and explicit empty completion.
      ```
    - Files to Create/Edit:
      - `test-plan/01-launch-and-connection.md`: Protocol v28 mixed-version behavior if protocol changes.
      - `test-plan/03-files-and-workspace.md`: Single-stream progressive open/reload/resync.
      - `test-plan/04-core-editing.md`: Typing/undo/history/remount behavior.
      - `test-plan/08-syntax-and-textobjects.md`: Viewport patch continuity and delayed syntax fallback.
      - `test-plan/11-performance.md`: Device/file/language matrix and targets.
      - `test-plan/13-window-splits.md`, `test-plan/14-tabs.md`: Multi-pane/tab routing and retention.
      - `test-plan/index.md`: Plan 099 coverage row and execution record.
      - `docs/development/tauri-react-parity-ledger.json`: Add every new manual step to the parity ledger.
      - `code-reviews/screenshots/2026-08-28-plan099-manual/manual-test-plan.md`: Store sanitized environment, report, RSS, and blocker evidence.
    - References:
      - `decision-logs/2026-08-04-1645-manual-test-plan-folder-and-per-plan-duty.md`
  - Test Cases to Write:
    - New numbered steps named above with expected result, negative check, and measured ceiling.
  - Completion Evidence:
    - Added and indexed Plan 099 steps: `L25–L26`, `F53–F54`, `E37–E38`, `S33–S34`, `M1–M7`, `D20–D21`, and `T77–T78` across `test-plan/01`, `03`, `04`, `08`, `11`, `13`, `14`, and `test-plan/index.md`; existing steps were not deleted or weakened.
    - Real Linux execution ran `scripts/editor-performance-smoke.sh --sizes 1,10,50 --kinds mixed-unicode,many-short-lines,long-lines,newline-heavy --label plan099-manual-20260828-181828 --enforce`. It generated 72 synthetic fixture variants, rebuilt the profiled frontend and binaries, launched Tauri/WebKit against a private server socket, inspected the real Clay frame through AT-SPI/screenshot, and closed it through its AT-SPI Close action.
    - Updated `docs/development/tauri-react-parity-ledger.json` with `L25–L26`, `F53–F54`, `E37–E38`, `S33–S34`, `D20–D21`, and `T77–T78`; the parity coverage guard passes.
    - The enforced harness passed with zero long tasks over 50 ms and bounded retention: frontend 18 retained/0 dropped, desktop 216/0, server 221/0. Bootstrap-only p95s were frontend open/ready 0/0 ms, desktop codec decode/encode 0.151/0.043 ms, server codec decode/encode 0.035/0.233 ms, and configuration load 14.056 ms. No syntax queue or viewport patch stage ran; `bridge.patch_delivery` was explicitly recorded as a warning rather than a false pass.
    - Supplemental idle RSS observation recorded server 58,188 KiB, client 13,876 KiB, desktop 211,888 KiB, and tracked total 283,952 KiB; it is documented as startup/idle RSS, not document-resident memory evidence. Host metadata records Linux 7.1.8-50.stable, GNOME Wayland, WebKitGTK 2.52.5, Ryzen 9 PRO 7940HS, Rust 1.96.1, and Node 24.19.0; no designated minimum-device run was available.
    - M1 is marked partial/unresolved for file opening; M2–M7 are explicitly unresolved because `uinput` was denied, `xdotool`/`ydotool` were absent, the RemoteDesktop portal exposed no input device, and WebKit document nodes were not exposed to AT-SPI. Retained final-build loading, four-pane routing, and typed-error artifacts are linked without substituting for missing keyboard-flow evidence.
    - Detailed sanitized record: `code-reviews/screenshots/2026-08-28-plan099-manual/manual-test-plan.md`; raw reports remain under `target/perf/editor-performance/plan099-manual-20260828-181828/`. Temporary fixture roots were removed and no user files, source text, credentials, or ambient paths were retained.
    - Validation: `cargo test --test protocol manual_smoke_docs` (25), `cargo test --test runtime editor_performance::editor_performance_matrix_holds_deterministic_invariants -- --exact` (1; 623.49 s), focused frontend performance tests (9), Prettier checks, and `git diff --check` passed.

- [x] Update performance, protocol, primitive, and package-authoring documentation
  - Acceptance Criteria:
    - Functional: Docs describe one current path for document ownership, incremental position mapping, viewport requests/patches, syntax sessions, Tauri coalescing, loading, diagnostics, folds, and fallback behavior.
    - Performance: Before/after evidence, operation-count invariants, device targets, parser/cache/queue bounds, and trace commands are documented without stale native-client benchmarks presented as current proof.
    - Code Quality: Primitive registry/index, protocol docs, architecture ownership, build/test guide, and package authoring contract match implementation; deterministic docs tests cover stable facts rather than incidental prose.
    - Security: Docs preserve server/package authority, internal budgets, trace privacy, and no client/package parser authority unless separately approved.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md` boundary: This task updates authoritative reference/development docs; final wiki task remains separate and occurs once.
      - `docs/reference/primitives/index.md`
      - `docs/reference/primitives/registry.md`
      - `docs/reference/primitives/parse-update-strategy.md`
      - `docs/reference/packages/creating-packages.md`
      - `docs/development/{performance,architecture-ownership,build-and-test}.md`
      - `docs/development/tauri-react-parity-ledger.json`
    - Options Considered:
      - Preserve historical descriptions beside current behavior: causes agents to implement removed paths.
      - Rewrite only changed current-state sections and retain dated history as clearly historical: chosen.
    - Chosen Approach:
      - Add/update generic primitives `BytePositionIndex`, `ViewportRenderPatch`, and `SyntaxSession` only after implementation. Keep package-facing contract unchanged and state that packages still publish inert data through validated server APIs.
    - API Notes and Examples:
      ```text
      BytePositionIndex: frontend-internal incremental position primitive
      ViewportRenderPatch: renderer-neutral internal protocol primitive
      SyntaxSession: server-internal per-document background work primitive
      ```
    - Files to Create/Edit:
      - `docs/reference/primitives/{index,registry,parse-update-strategy,rendering-strategy}.md`
      - `docs/reference/packages/creating-packages.md`
      - `docs/development/{performance,architecture-ownership,build-and-test,file-open-save-reload-workflow}.md`
      - `docs/development/tauri-react-parity-ledger.json`
      - `docs/index.md`: Refresh master navigation descriptions for current Plan 099 references.
      - `tests/primitives_docs.rs`, `tests/documentation_coverage.rs`: Stable primitive/path/budget coverage.
      - `.agents/skills/clay-ui/references/components.md`: Update editorView internal notes only if implementation contract changes; no new kind expected.
      - `.agents/skills/clay-ui/references/tokens.md`: Update only if token consumption changes; no new token expected.
    - References:
      - `.agents/skills/project-patterns/references/documentation-as-code.md`
      - `.agents/skills/project-patterns/references/maintenance-validation.md`
  - Test Cases to Write:
    - Primitive documentation coverage fails when source/registry/reference/wiki links drift.
    - No current doc claims full-document per-edit index rebuild, heuristic viewport acknowledgement, duplicate session, or Tokio-native parser execution.
    - Package guide confirms no new raw UI/parser/Tauri authority.
  - Completion Evidence:
    - Updated the current reference/development contract across `docs/reference/primitives/{index,registry,parse-update-strategy,rendering-strategy}.md`, `docs/development/{performance,architecture-ownership,build-and-test,file-open-save-reload-workflow}.md`, and `docs/index.md`. The docs now identify `BytePositionIndex`, protocol-v29 `ViewportRenderPatch`, and per-document `SyntaxSession` as implemented internal primitives with their owners, data flow, bounds, fallback, and non-authority boundaries.
    - Updated `docs/reference/packages/creating-packages.md` with the Plan 099 authoring contract: existing server registration/publication facades remain the package boundary; packages receive no client `Text`/position index, patch completion, parser handle, scheduler control, raw op, callback, CSS, or client parser authority. The parity ledger records the three implemented primitives under editing, intelligence, and performance capabilities.
    - Added deterministic documentation guards: `primitives_docs::plan099_editor_documentation_matches_current_implementation` checks stable identifiers, source paths, package boundaries, and removal of stale claims; `documentation_coverage::plan099_reference_docs_are_cross_linked_and_current` checks master/reference links and current-state wording. Both pass.
    - `cargo run --bin update-doc-registry` left the generated Clay JS registry unchanged because no public API changed. `cargo fmt --check`, `cargo check --all-targets`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets` (1151 library, 6 presentation, 30 editor, 196 protocol, 71 runtime, 130 security), JSON validation, and `git diff --check` passed.

- [x] Update or verify the code wiki after implementation
  - Acceptance Criteria:
    - Functional: Code wiki is updated once after all implementation, verification, API/configuration, manual-test, and reference-doc tasks pass.
    - Performance: Wiki explains exact hot paths, state ownership, complexity, bounds, metrics, fallback, and before/after evidence without adding runtime work.
    - Code Quality: Pages explain implementation responsibilities, flow, data structures, cancellation, invariants, source/test paths, examples, extension guidance, and links from master index.
    - Security: Pages document validation, package provenance, trace privacy, and authority boundaries without exposing secrets.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/project-wiki/SKILL.md`
      - `.agents/skills/create-plan/references/wiki-task.md`
    - Options Considered:
      - Update after every task: noisy and likely to document intermediate architecture.
      - Update once after final checks: chosen.
    - Chosen Approach:
      - Update existing pages and create only pages needed to teach new stable primitives/flow. Keep `docs/wiki/index.md` navigable.
    - API Notes and Examples:
      ```text
      docs/wiki/modules/react-codemirror-editor.md
      docs/wiki/modules/parse-coordinator.md
      docs/wiki/flows/editor-viewport-render-patch.md
      ```
    - Files to Create/Edit:
      - `docs/wiki/index.md`: Link all changed/new implementation pages.
      - `docs/wiki/modules/react-codemirror-editor.md`: Single text owner, incremental index, bounded render fields.
      - `docs/wiki/modules/desktop-typed-bridge.md`: Whole-patch coalescing and trace boundary.
      - `docs/wiki/modules/parse-coordinator.md`: Syntax-session registration/output validation role.
      - `docs/wiki/modules/decoration-transport.md`: Atomic covered-range patch flow.
      - `docs/wiki/modules/folding-ranges.md`, `docs/wiki/modules/range-diagnostics.md`: Incremental interval projection.
      - `docs/wiki/flows/frontend-edit-synchronization.md`: No full-index scan/React work on edit.
      - `docs/wiki/flows/document-chunked-loading.md`: One request stream, one text owner, no-history chunks.
      - `docs/wiki/flows/editor-viewport-render-patch.md`: New request-to-patch flow if protocol task is approved and implemented.
    - References:
      - `.agents/skills/project-wiki/SKILL.md`
  - Test Cases to Write:
    - Wiki navigation/source-path guard passes.
    - Manual wiki review confirms current source/test paths and no stale duplicate architecture.
  - Completion Evidence:
    - Updated and indexed the current Plan 099 implementation pages: `react-codemirror-editor`, `desktop-typed-bridge`, `parse-coordinator`, `parse-task-lifecycle`, `syntax-sessions`, `decoration-transport`, `range-diagnostics`, `folding-ranges`, `performance-fixtures`, `protocol-codec`, `react-client-bridge`, `frontend-edit-synchronization`, and `document-chunked-loading`.
    - Added `docs/wiki/flows/editor-viewport-render-patch.md`, documenting the v29 request-id flow from indexed visible bytes through server validation, `SyntaxSession`, request-scoped aggregation, whole-patch Tauri coalescing, and one CodeMirror transaction. The master `docs/wiki/index.md` now links this flow and describes current one-`Text`, incremental-index, patch, and session ownership.
    - Wiki pages now list current source/test paths, complexity and bounds, stale-result/cancellation behavior, trace privacy, package authority, fallback behavior, and deterministic extension guidance. Removed current-state claims for synthetic viewport acknowledgements, duplicate/full-document browser state, and per-member patch coalescing; historical pages remain explicitly historical.
    - Extended `documentation_coverage::wiki_navigation_is_complete_and_current_page_paths_resolve` to cover the new viewport-patch and chunk-loading flow pages. `cargo test --test protocol wiki` passed (4 tests), including every-page index/link resolution, current source-path validation, and Plan 088 wiki compatibility markers.
    - Targeted Prettier checks passed for all changed wiki pages; `git diff --check` passed. No runtime or public API changes were introduced.

## Compromises Made

- Stock Lezer was rejected at the cheapest grammar gate; browser latency, viewport, memory, pane, and reproducibility evidence was intentionally not collected.
- The disposable Lezer editor integration, dependencies, recorder, and runner were removed after preserving the dated report and decision evidence.
- Plan 099 instrumentation has one reference-host Rust baseline and deterministic browser/schema coverage; interactive WebKit timing remains uncollected because this host lacks `ydotool` input automation. Minimum-device promotion still requires three stable runs.
- Visual review used synthetic Tauri/WebKit runs and bounded source-free traces. Keyboard-only focus, typing, and rapid-scroll evidence remain unresolved on this Wayland host because no keyboard-capable input backend is available.
- The real Markdown diagnostics state did not show a visible Marksman marker after the bounded capture and confirmation wait; native loading/error/recovery fixture states likewise remain explicitly unresolved in the review artifact rather than being called passes.
- The Plan 099 manual matrix generated and launched all 72 fixture variants, but this host could not drive file open/edit/scroll/fold/save/resync. Its enforced result is bootstrap-only evidence; M1 is partial and M2–M7 remain open for an input-capable designated device.
- The long-form legacy Markdown reference files remain in their pre-existing non-Prettier style; targeted Prettier checking reports that baseline drift. Whole-file reformatting was deferred to avoid unrelated documentation churn.

## Further Actions

- Keep Plan 099 M2–M7 open until an input-capable designated-device run exists; wiki implementation coverage is complete for the current architecture.
- Re-run the real Markdown diagnostics state after analyzer startup/reset behavior is made deterministic, and refresh native loading/error/recovery captures once the harness observes current Tauri/React runtime snapshots.
- Repeat keyboard-only focus, typing, and scroll review on an input-capable Linux runner; do not promote current reference-host timings to minimum-device gates.
- Keep server-side per-document Tree-sitter sessions, atomic viewport patches, and one-base-owner authority as the active direction.
- Reconsider a client parser only after the completed server-session path misses approved metrics and traces attribute the remaining delay to server/bridge placement.
- Clean the 112 GiB `target/` only after preserving any needed benchmark artifacts and ensuring no active build uses it.
