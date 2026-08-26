# Clay Editor Performance Overhaul

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
- Client-local parsing remains deferred unless post-overhaul measurements justify a separately approved decision.

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

- [ ] Add correlated editor performance instrumentation and establish production baselines
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
      - Extend `CLAY_PERF_PROFILE` through the Tauri launch boundary and expose a development-only in-memory frontend recorder. Add trace IDs only to internal performance envelopes or developer events, not public package APIs.
    - API Notes and Examples:
      ```ts
      perf.mark("editor.localUpdate", {
        traceId,
        documentId,
        version,
        durationNs,
      });
      ```
    - Files to Create/Edit:
      - `frontend/src/editor/performance.ts`: Bounded disabled-by-default recorder and trace summaries.
      - `frontend/src/editor/create-editor.ts`: Input/local-update and measure-adjacent marks.
      - `frontend/src/editor/extensions/controller.ts`: Viewport request/patch marks.
      - `frontend/src/shell/workspace-controller.ts`: Open/head/ready markers.
      - `src-tauri/src/bridge/session.rs`: Request/delivery correlation markers.
      - `src/perf/metrics.rs`: Shared numeric metric names and bounded snapshots.
      - `src/server/connection/documents.rs`: Receive/ack/enqueue markers.
      - `src/server/parse_coordinator.rs`, `src/server/syntax.rs`: Queue/start/end/publish markers.
      - `docs/development/performance.md`: Trace command, schema, privacy boundary, and baseline table.
    - References:
      - `docs/development/editor-performance-review-2026-08-26.md`
  - Test Cases to Write:
    - Disabled recorder test: zero retained frontend/server events and no trace payload work.
    - Capacity test: bounded ring drops excess and records drop count.
    - Source-safety test: serialized trace contains no fixture text/path/secret markers.
    - Correlation test: one synthetic viewport trace contains every expected stage in monotonic order.

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

- [ ] Fix single-document ownership, progressive loading, and React subscription isolation
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

- [ ] Replace immutable-document position cache with an incremental CodeMirror position primitive
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
        update: (index, transaction) => index.apply(transaction.changes, transaction.state.doc),
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

- [ ] Implement atomic, viewport-bounded frontend decorations, diagnostics, and folds
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

- [ ] Replace viewport heuristics and bridge member coalescing with an explicit atomic patch protocol
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

- [ ] Move syntax and mode activation off connection/Tokio hot paths into bounded per-document sessions
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

- [ ] Run automated and real-device editor performance matrix and enforce stable invariants
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
    - 100-scroll patch retention test remains constant-size.
    - 50 MiB one-session/no-history/one-current-Text invariant.
    - Four-pane aggregate patch/work count remains linear in visible panes.
    - Software-rendering smoke remains functional without lost text/highlights.

- [ ] Decide whether a client-local parser spike is still necessary
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
      - Close task as “not needed” when targets pass. Otherwise produce measured spike report and request explicit approval before decision/implementation.
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
  - Test Cases to Write:
    - Spike parity matrix, if triggered: Syntax captures, incremental edits, large-file fallback, package provenance, theme tokens, headless/server behavior, bundle/memory cost.

- [ ] Perform visual screenshot and accessibility review of changed editor states
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
      - `frontend/src/routes/fixture.tsx`: Deterministic performance states only when real state cannot be reliably held for capture.
      - `docs/wiki/modules/ui-review-harness.md`: Performance capture flow.
      - `code-reviews/screenshots/<date>-plan099-editor-performance/`: Screenshots, semantic dumps, trace summaries, review log.
    - References:
      - `.agents/skills/clay-ui/SKILL.md` visual/a11y proof rules.
  - Test Cases to Write:
    - Keyboard-only focus reaches editor after ready; loading gate has named status; error remains actionable.
    - Screen reader sees current editable document only, not duplicate hidden session text.
    - Four-pane scroll/focus semantics remain distinct and responsive.

- [ ] Create or verify Clay JS APIs for public programmatic surfaces
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
      - `docs/reference/clay-js-api/api-inventory.toml`
      - `docs/reference/primitives/registry.md`
    - Options Considered:
      - Expose syntax scheduler tuning: rejected; server-owned performance/security budgets.
      - Expose raw viewport patch requests to packages: rejected; client/server internal render transport.
      - Verify-only unless implementation introduces genuine public behavior: chosen.
    - Chosen Approach:
      - Inventory changed Rust visibility and keep performance plumbing internal.
    - API Notes and Examples:
      ```bash
      cargo test --test protocol clay_js_api
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/api-inventory.toml`: Only if intentional API changes.
      - `docs/reference/clay-js-api/**/*.md`: Only for intentional API changes.
      - `docs/index.md`: Link any intentional API docs.
      - Generated registry artifacts: Update through documented command if docs change.
    - References:
      - `decision-logs/2026-05-08-1509-clay-js-api-facade-for-rust-functions.md`
      - `decision-logs/2026-05-08-1840-clay-js-api-discovery-keybindings-custom-properties.md`
  - Test Cases to Write:
    - API inventory/registry/docs guards pass.
    - Source visibility review: New scheduler/patch helpers are not public without facade/docs.

- [ ] Create or verify Clay configuration APIs
  - Acceptance Criteria:
    - Functional: Review confirms parser concurrency, caches, overscan, request pacing, trace capacity, and device budgets remain host-owned unless explicit user value is proven.
    - Performance: No configuration option can raise hard memory/frame/concurrency limits into unsafe ranges or force synchronous/full-document syntax.
    - Code Quality: Any user-visible preference is a documented Clay JS API through `~/.config/clay/init.js`; no hidden environment/config key becomes normal product behavior.
    - Security: Configuration grants no new filesystem, network, shell, process, parser artifact, Tauri, package, or AI authority.
  - Approach:
    - Documentation Reviewed:
      - `.agents/skills/create-plan/references/clay.md`
      - `.agents/skills/project-patterns/references/configuration-system.md`
      - `docs/reference/clay-js-api/configuration.md`
      - `examples/init.js`
      - `src/perf/budgets.rs`
    - Options Considered:
      - User-tunable low-level performance knobs: rejected until a concrete need cannot be auto-managed.
      - Developer-only opt-in trace environment: retained, bounded, not user configuration.
      - Verify-only configuration task: chosen.
    - Chosen Approach:
      - Keep scheduler/cache/patch budgets compiled and measured. Update canonical example only if an approved public preference is introduced.
    - API Notes and Examples:
      ```text
      Expected result: no new init.js option for syntax threads, patch size, cache bytes, or chunk pacing.
      ```
    - Files to Create/Edit:
      - `docs/reference/clay-js-api/configuration.md`: Clarify only if behavior contract changes.
      - `examples/init.js`: Update only if an approved public option lands.
    - References:
      - `decision-logs/2026-05-08-1841-configuration-through-init-js-and-clay-js-apis.md`
  - Test Cases to Write:
    - Closed configuration surface test rejects internal performance/security budget names.
    - `node --check examples/init.js` and canonical-example cross-check pass.

- [ ] Execute and update the manual test plan
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
    - Options Considered:
      - Automated only: misses physical WebKit/device behavior.
      - Unstructured manual notes: not repeatable.
      - Extend numbered modules and record exact environment/results: chosen.
    - Chosen Approach:
      - Execute after automated and visual checks so manual work validates final build.
    - API Notes and Examples:
      ```text
      Q38: 1 MiB many-short-lines, 100 edits, zero full-index rebuilds, local-update p95 within target.
      Q39: 10 MiB Rust fling scroll, explicit viewport patch IDs, no >50 ms long task.
      ```
    - Files to Create/Edit:
      - `test-plan/01-launch-and-connection.md`: Protocol v28 mixed-version behavior if protocol changes.
      - `test-plan/03-files-and-workspace.md`: Single-stream progressive open/reload/resync.
      - `test-plan/04-core-editing.md`: Typing/undo/history/remount behavior.
      - `test-plan/08-syntax-and-textobjects.md`: Viewport patch continuity and delayed syntax fallback.
      - `test-plan/11-performance.md`: Device/file/language matrix and targets.
      - `test-plan/13-window-splits.md`, `test-plan/14-tabs.md`: Multi-pane/tab routing and retention.
      - `test-plan/index.md`: Plan 099 coverage row and execution record.
    - References:
      - `decision-logs/2026-08-04-1645-manual-test-plan-folder-and-per-plan-duty.md`
  - Test Cases to Write:
    - New numbered steps named above with expected result, negative check, and measured ceiling.

- [ ] Update performance, protocol, primitive, and package-authoring documentation
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

- [ ] Update or verify the code wiki after implementation
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

## Compromises Made

- Pending execution. Current recommendation deliberately defers client-local parsing to avoid owning a second parser/package architecture before optimized server syntax is measured.

## Further Actions

- After Plan 098 is committed, rebase file/line references before Plan 099 implementation.
- Clean the 112 GiB `target/` only after preserving any needed benchmark artifacts and ensuring no active build uses it.
- Revisit parser placement only through the metric gate task; do not add language packages or worker WASM speculatively.
