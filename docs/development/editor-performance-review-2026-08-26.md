# Clay editor performance review

**Review date:** 2026-08-26

**Repository state:** commit `03a7fae` plus current uncommitted Plan 098 working tree

**Scope:** file open and progressive transfer, React/CodeMirror ownership, UTF-8/UTF-16 position conversion, syntax/semantic decorations, diagnostics, folds, viewport scheduling, Tauri forwarding, server connection dispatch, parse coordination, Tree-sitter execution/cache behavior, tests, metrics, and device validation.

## Executive summary

Clay's file-size ceiling is no longer the main editor bottleneck. Plan 098 makes disk read, wire transfer, and save bounded, but current editor still performs document-sized work on the browser main thread and sends syntax through a fragmented, weakly coordinated server round trip.

Five shared choke points explain why every file type can stall:

1. **Every edited CodeMirror document invalidates the UTF position cache.** `textIndex` rebuilds line starts, UTF-8 lengths, and copied line strings for the entire immutable `Text`. The real edit path calls it on every user change. A review-only 1 MiB measurement on the high-end development host took **18.0-25.8 ms per rebuild**, already above Clay's 16 ms frame budget before CodeMirror layout, React, IPC, or syntax work.
2. **Production creates two document sessions for the same tab.** The workspace pane session and obsolete global singleton both install bootstrap, request document chunks, retain document text, and consume every routed event. Current loading therefore duplicates requests and retains three logical frontend document copies: pane session shadow, CodeMirror document, and global singleton shadow.
3. **Decoration application scales with retained history, not visible work.** Server syntax uses 128-byte authority chunks. Frontend turns every chunk into a separate effect, clones a growing map once per effect, rebuilds every retained mark/link after each batch, and never prunes old viewport chunks. Diagnostics rebuild all retained diagnostics with an O(N²) suppression pass. Folding scans all fold ranges for each queried line.
4. **Expensive syntax preparation remains in the connection path.** Open sends the head, then synchronously performs runtime mode activation and analysis before the connection can serve the next chunk request. Edit sends its acknowledgement, then copies a parse window and schedules syntax before accepting the next message. Tree-sitter itself runs synchronously inside `tokio::spawn`, blocks a Tokio worker, and is serialized by one parser mutex per grammar.
5. **Viewport transport has no explicit request completion contract.** Any decoration, diagnostic, or fold event releases the frontend inflight gate. Tauri coalesces `DecorationSet` without viewport bounds and `DecorationBatch` by document only, so sibling windows or independent package batches can overwrite one another. A 400 ms timer hides lost/no-result cases instead of proving completion.

These are architecture and algorithm issues, not GPU styling issues. CSS shows no large blur/filter/shadow or layout-animation hotspot in editor surfaces.

**Recommended direction:** keep server-authoritative syntax initially, but rebuild hot-path ownership around one CodeMirror document, an incremental byte-position index, one atomic viewport patch, explicit request IDs, and a bounded per-document syntax scheduler off connection/Tokio worker paths. Do not add a second client parser yet. Run a CodeMirror/Lezer or worker parser spike only if measured optimized server publication still misses the latency target.

## Evidence and measurements

### Host

- AMD Ryzen 9 PRO 7940HS, 8 cores / 16 threads
- 61 GiB RAM
- Linux 7.1.8
- WebKitGTK 2.52.5
- Node 24.19.0
- Rust 1.96.1

This is a fast development machine. A main-thread budget miss here is more severe on minimum hardware.

### Review-only frontend measurements

Command used a temporary Vitest/jsdom test that was deleted after execution.

| Scenario | Result | Meaning |
| --- | ---: | --- |
| Rebuild `textIndex` after each immutable edit, 1 MiB / 61,681 lines | 18.0-25.8 ms per edit | Position conversion alone exceeds 16 ms frame budget. |
| Existing `performance.test.ts` bare 1 MiB mount + one edit | test 118 ms total | Test does not install Clay's user-change listener or call `textIndex`; it misses real hot path. |
| Existing 1,000-span decoration test | test 27 ms | Fixture document is about 30 KiB and threshold is 500 ms; it does not model chunk accumulation, scrolling, bridge delivery, or WebKit layout. |
| Review batch: first 128-chunk decoration install | 31.9 ms | Main-thread batch can exceed one frame before real WebKit paint. Later samples varied with JIT/GC and retained-map size. |

### Existing Plan 098 measurements

Current Plan 098 protocol-level 50 MiB test records:

- open to head: about 297 ms
- open to complete transfer: about 589 ms
- save to acknowledgement: about 423 ms

These prove bounded server transfer. They do **not** include a verified production WebKit CodeMirror load/scroll trace. The manual Tauri editor walkthrough remained unresolved because of compositor/AT-SPI limitations. Consequently, Plan 098 cannot establish GUI responsiveness.

### Current observability gap

Rust records syntax parser/query/publication metrics. Frontend and Tauri do not record correlated milestones for input, CodeMirror update, React commit, viewport request, bridge delivery, patch application, layout, or paint. No current trace can attribute a visible pause to browser, bridge, server queue, parser, or decoration application.

## Current end-to-end flow

### Open

1. Server streams file into a Crop rope under resident-memory and binary guards.
2. `DocumentOpened` sends a bounded head.
3. Frontend pane session paints head and requests one 256 KiB chunk at a time.
4. Obsolete global session independently repeats the same requests and assembly.
5. Server connection writes the head, then awaits mode classification, generated V8 module evaluation, parse setup, and analysis startup.
6. Chunk requests wait in the incoming queue until follow-up work returns.
7. Each chunk appends to a session `Text` and separately dispatches another string append into CodeMirror.
8. Programmatic chunk transactions do not carry CodeMirror's `Transaction.addToHistory.of(false)` annotation.
9. When complete, metadata rerenders React and reconfigures read-only state.
10. Editor issues viewport syntax request; server parses/query-captures and returns fragmented decoration sets plus folds.

### Typing

1. CodeMirror applies local transaction.
2. Clay converts edit offsets through `textIndex(transaction.startState.doc)`.
3. New immutable `Text` misses WeakMap cache, causing full-document line/UTF scan and line-string retention.
4. Session sends edit and updates metadata `pending`.
5. Metadata update rerenders `ClayEditor`, notifies whole workspace, schedules layout persistence, and reconfigures CodeMirror read-only even when value did not change.
6. Server applies edit and writes acknowledgement.
7. Before handling next client message, server obtains runtime/workspace/document state, copies parse window text, and schedules syntax.
8. Ack updates metadata again, repeating React/workspace/reconfiguration work.
9. Syntax result replaces the frontend decoration version. First current-version set clears prior chunks, so incomplete replacement can temporarily remove most visible highlighting.

### Scrolling

1. Every exact visible-range change creates a viewport key.
2. One request is inflight; later ranges collapse to one pending flag.
3. Server splits large visible requests into up to 24 windows and spawns one task per window.
4. Each task runs synchronous Tree-sitter work on Tokio, under shared grammar parser mutex.
5. Each update emits 128-byte authority chunks and recomputes a folding set.
6. Tauri may coalesce sibling batches by document.
7. Frontend treats any decoration/diagnostic/fold event as request completion.
8. Frontend installs N effects, clones maps repeatedly, reprojects all retained spans, and retains offscreen chunks indefinitely.

## Ranked findings

## P0 - direct causes of editor stalls

### P0-1: Position index is O(document) per edit and retains duplicated line text

**Evidence**

- `frontend/src/editor/position-map.ts:39-69`
- `frontend/src/editor/sync/operations.ts:29`
- `frontend/src/editor/extensions/controller.ts:402`
- `frontend/src/editor/extensions/decorations.ts:76`

`indexCache` is keyed by immutable CodeMirror `Text`. Every document edit creates a new `Text`, so the next conversion rebuilds the entire index. The index stores:

- one UTF-16 start per line;
- one UTF-8 start per line;
- one UTF-8 length per line; and
- a copied JavaScript string for every line.

CodeMirror history can retain old `Text` roots, which in turn keeps WeakMap keys and their full indices reachable. Cost is therefore CPU plus potentially large history-amplified memory.

**Impact**

- Shared by all file types.
- Linear file-size typing cost.
- First conversion after every edit can block main thread beyond one frame.
- Many-short-lines fixtures amplify array/string overhead.
- Very long lines still require an intra-line scalar scan.

**Required fix**

Move byte-position state into one CodeMirror `StateField` or equivalent persistent order-statistic structure. Build once at document install, update only touched lines/ranges from `transaction.changes`, and expose O(log lines + touched-line) conversion. Do not cache complete copied line text per historical immutable document.

### P0-2: Duplicate production document session doubles transfer and retains dead state

**Evidence**

- `frontend/src/app/use-clay-session.ts:83-94,124-125`
- `frontend/src/shell/workspace-controller.ts:181`
- `frontend/src/editor/session-singleton.ts`

`workspace.installBootstrap` creates and installs the real pane session. The same hook then calls `documentSession.installInitial`. Every routed envelope is sent to workspace and again to `documentSession`.

**Impact**

- Two chunk request streams for same document.
- Two independent progressive assemblers.
- Duplicate status requests and event processing.
- Global singleton retains up to 256 feature envelopes despite not rendering production editor.
- Current pane separately retains `authoritativeDoc` and CodeMirror document, producing three logical frontend copies before history.

**Required fix**

Remove singleton from production bridge lifecycle. Keep test-only construction if useful. Make pane session/CodeMirror share one current `Text` reference rather than maintaining a stale second live copy.

### P0-3: Progressive-load transactions can populate history and duplicate every chunk

**Evidence**

- `frontend/src/editor/sync/session.ts:139-141,205-210`
- `frontend/src/editor/create-editor.ts:85`

For each chunk, session calls `Text.of(value.split("\n"))`, appends it to `authoritativeDoc`, then dispatches original string into CodeMirror, which parses it again. Transactions carry Clay origin only, not CodeMirror's no-history annotation.

`authoritativeDoc` is not updated after user edits. `paintAuthoritative` compares length only, so same-length reload/resync can leave stale text visible.

**Impact**

- Duplicate chunk parsing/allocation.
- Full second document shadow after ready.
- Potential hundreds of load transactions retained by history for 50 MiB file.
- Stale remount/reload behavior can cause replacement and re-layout surprises.

**Required fix**

One document owner. If view exists, dispatch one no-history append and set session snapshot to `view.state.doc` by reference. If view is detached, append once to session `Text`. Use explicit generation/content identity, never length equality. Full replace/resync must reset or isolate history.

### P0-4: Decoration application is O(effects x chunks + retained spans) and unbounded by viewport

**Evidence**

- `src/server/syntax.rs:11` (`SYNTAX_DECORATION_CHUNK_BYTES = 128`)
- `frontend/src/editor/extensions/controller.ts:246-254`
- `frontend/src/editor/extensions/decorations.ts:169-191`

A normal visible/guard window produces dozens of 128-byte `DecorationSet`s. Controller creates one effect per set. State field clones `chunks` for each effect. After all effects, `project` walks every retained chunk/span, rebuilds all style strings, sorts all marks, and recreates full CodeMirror `DecorationSet`. No frontend near-viewport eviction exists.

The same data is retained twice: `EditorProjection.decorationSets` and decoration field `chunks`.

**Impact**

- Scroll cost grows for lifetime of document/version.
- Map cloning is quadratic within large batches.
- Style attributes are rebuilt as strings repeatedly.
- Remount replays up to 256 historical feature envelopes, each through new dispatch.
- Exact 128-byte chunks optimize old native replacement logic at cost of excessive JSON/DOM objects in React target.

**Required fix**

Introduce one atomic `DecorationPatch` effect containing complete covered ranges and spans. Clone/update state once. Use CodeMirror `RangeSet` mapping/update to remove only covered ranges and add only new marks. Retain visible + measured overscan only. Predefine token classes instead of generating repeated inline style strings. Remove duplicate controller cache.

### P0-5: Open and edit connection handlers contain syntax/runtime follow-up work

**Evidence**

- `src/server/connection/documents.rs:58-86`
- `src/server/connection/documents.rs:173-214`
- `src/server/connection/documents.rs:532-694`
- `src/server/connection/documents.rs:695-747`

Open writes first head, then awaits classification, controlled V8 evaluation, parse setup, and analysis startup before returning to connection `select`. Chunk requests cannot be dispatched during this interval.

Edit writes `EditAck`, then obtains multiple locks, creates a parse snapshot string, and schedules syntax before accepting next incoming edit. Local paint stays optimistic, but server acknowledgements can backlog and `pending` grows.

**Impact**

- Head appears, remainder pauses.
- First open of a mode can wait behind package loading/evaluation.
- Rapid typing can queue behind parse-window copies.
- One connection's control flow mixes authority mutation with advisory analysis.

**Required fix**

After canonical open/edit and required response, enqueue lightweight latest-wins document work to a server-owned per-document scheduler. Connection loop must return immediately. Chunk serving, edit acceptance, save/close, and resync remain priority. Advisory syntax/analysis publishes through existing bounded output routers.

### P0-6: Tree-sitter blocks Tokio and parser ownership serializes documents

**Evidence**

- `src/server/syntax.rs:1374-1382`
- `src/server/syntax.rs:2341-2348`
- `src/server/parse_coordinator.rs:694`

Coordinator uses `tokio::spawn`, but native handler's async future directly calls synchronous `parse_sync`. Parser and tree maps use standard mutexes. One parser mutex per grammar serializes all documents using that grammar while occupying Tokio workers.

Tree cache stores only one `CachedSyntaxTree` per document, including one `window_id`. Scrolling large files across windows overwrites prior cached window, causing repeated fresh parses when returning.

**Impact**

- CPU parse can delay unrelated async server tasks.
- Same-language documents contend globally.
- Up to 24 viewport windows can create a queued parse storm.
- Large-file scroll revisits lose cached tree reuse.

**Required fix**

Use a bounded blocking syntax executor. Prefer one per-document syntax session/actor that owns parser state and latest requested viewport/version. Keep at most one active parse per document/grammar, cancel/coalesce older work, and cache a small measured number of stable large-file windows. Share immutable `Language`/`Query` metadata, not mutable parser instances.

## P1 - syntax continuity and scroll latency

### P1-1: Viewport request completion is inferred from unrelated messages

**Evidence**

- `frontend/src/editor/extensions/controller.ts:226-323,438-457`

`viewportReply` becomes true for any decoration, diagnostic, or fold event. These events have no request ID. A stale or independent analysis event can release current viewport gate. A valid request with no grammar/no changes waits 400 ms timer.

**Required fix**

Version viewport requests with `requestId`/generation. Server returns exactly one `ViewportPatchComplete` or one atomic patch with same ID, including empty result. Client accepts only latest compatible response and immediately schedules newest pending viewport.

### P1-2: Tauri latest-wins key can drop required sibling state

**Evidence**

- `src-tauri/src/bridge/forwarder.rs:30-43`

`DecorationSet` key omits viewport bounds. `DecorationBatch` key is only `batch|document_id`. Separate windows, packages, kinds, or layers can overwrite before drain.

**Required fix**

Coalesce whole request-scoped atomic patches, keyed by client/document/feature/request generation. Never coalesce individual members of one complete patch. Until protocol change, include package, kind, version, and viewport in key as correctness hotfix.

### P1-3: Server computes and transports folds on viewport syntax work

**Evidence**

- `src/server/syntax.rs:1734-1740`
- `frontend/src/editor/extensions/folding.ts:51-66`

Every syntax parse, including scroll viewport parse, walks tree to produce folds. Frontend fold service scans every range in every set for each queried line.

**Required fix**

Publish fold changes only when document version/tree changes or explicit fold coverage changes, not on same-version viewport queries. Store sorted ranges and binary-search by line start in frontend.

### P1-4: Diagnostic projection has O(N²) suppression and full replacement

**Evidence**

- `frontend/src/editor/extensions/diagnostics.ts:32-51`

Every set flattens all chunks, computes suppressors, then calls `some(overlaps)` for each Tree-sitter span. It rebuilds complete lint diagnostics and has no near-viewport pruning.

**Required fix**

Keep source/viewport keyed sorted interval sets, discard replaced/offscreen chunks, merge only changed source ranges, and use an interval sweep rather than nested overlap search. Avoid `setDiagnostics` full-list churn when patch unchanged.

### P1-5: Feature replay is count-bounded, not byte- or key-bounded

**Evidence**

- `frontend/src/editor/sync/session.ts:90,321-322`

256 envelopes can include large decoration batches, diagnostics, folds, completion results, and intelligence payloads. Replay preserves historical superseded versions rather than latest state.

**Required fix**

Retain latest keyed state per document/version/package/kind/range under explicit byte budget. Do not retain request-scoped completion/intelligence results for remount.

### P1-6: Mode activation evaluates generated JavaScript on document open

**Evidence**

- `src/server/connection/documents.rs:583-662`

Each open constructs source text and calls `evaluate_controlled_module_for_document`. On no non-core classification it loops first-party package specifiers and attempts loads. This runs after head but before connection returns.

**Status:** structurally synchronous, but not yet measured separately in production.

**Required fix**

Instrument first. Likely end state: runtime generation installs a validated static mode/grammar registry snapshot; document open performs Rust-side classification against that snapshot and sends an asynchronous activation intent only when package runtime behavior must change. No generated module per ordinary open.

## P1 - React and CodeMirror integration

### P1-7: Every pending/ack metadata change triggers broad React and CodeMirror work

**Evidence**

- `frontend/src/editor/ClayEditor.tsx:95-100`
- `frontend/src/shell/workspace-controller.ts:125-133`

Every store update:

- rerenders `ClayEditor`;
- runs read-only compartment reconfigure because effect depends on whole `meta` object;
- recomputes tab dirty state;
- calls `tabs.set`;
- notifies whole workspace/pane tree; and
- schedules layout persistence.

A normal edit causes at least pending and ack updates.

**Required fix**

Subscribe with derived selectors. Reconfigure read-only only when boolean changes. Separate fast editor metadata from workspace topology/persistence. Persist only path/tree/dirty transitions, not pending/version acknowledgements. Batch visible pending/version chrome to animation frame or low-rate status updates if needed.

### P1-8: Behavior/caret/layout updates rebuild broad extension compartments

`applyCaret` and `applyLayout` reconfigure the same behavior compartment with full behavior extensions. Manifest install also rebuilds keymaps/chrome plugins. This is rare but can recreate gutters, indent plugins, and themes together.

**Required fix**

Use separate compartments for behavior rules, chrome, wrap, caret, and keymap. Reconfigure only changed facet. Keep this after hot-path fixes unless traces show frequent runtime churn.

## P2 - testing, metrics, and development feedback

### P2-1: Current performance tests do not execute Clay hot paths

- Bare 1 MiB typing test omits Clay update listener and position map.
- Decoration test uses small document, one batch, no accumulated scroll state, no React, no bridge, no WebKit.
- Rust work-count tests validate parser calls/payloads but not Tokio starvation, connection-loop latency, or 24-window concurrency.
- Plan 098 large-file test does not mount production CodeMirror/WebKit.
- No render-count test covers pending + ack workspace notifications.

**Required fix**

Add deterministic operation-count/memory-state guards plus browser/Tauri traces. Do not use loose jsdom wall-clock thresholds as sole CI gate.

### P2-2: Metrics cannot correlate one user-visible stall

Add a numeric trace ID across:

- browser input/viewport event;
- CodeMirror transaction completion;
- frontend request enqueue;
- Tauri request parse/enqueue;
- server receive/ack;
- syntax queue/start/end;
- bridge delivery;
- frontend patch apply;
- CodeMirror measure/paint proxy.

Production metrics remain disabled by default and content-free.

### P2-3: Build artifacts are 112 GiB

This does not cause editor runtime stalls, but it slows iteration and encourages overlapping verification. Keep one target directory, retain quick/full locks, and clean stale artifacts after current Plan 098 work is safely committed.

## Target architecture

```text
CodeMirror input / scroll
        |
        | local transaction, no React state
        v
BytePositionIndex StateField  <--- incrementally mapped by transaction changes
        |
        +--> ordered edit delta queue ----------------------------+
        |                                                         |
        +--> latest viewport request { requestId, version, range } |
                                                                  v
Tauri narrow bridge --> server connection priority dispatcher --> canonical rope/edit ack
                                                        |
                                                        +--> per-document latest-wins syntax session
                                                              |
                                                              | bounded blocking executor
                                                              v
                                                        Tree-sitter tree/window cache
                                                              |
                                                        atomic ViewportPatch
                                                              |
Tauri coalesces patch by request identity <--------------------+
        |
        v
CodeMirror DecorationPatch StateField
  - one state update
  - remove covered ranges
  - add new ranges
  - visible + overscan retention
  - no broad React rerender
```

### Ownership

- **CodeMirror:** current frontend `Text`, selections, history, viewport, incremental byte index, inert visible decorations.
- **React:** chrome and coarse metadata only.
- **Tauri Rust:** validated transport/OS bridge only; no document shadow.
- **Server:** canonical Crop rope, versions, leases, mode/package authority, parser scheduling, semantic/LSP authority.
- **Syntax session:** advisory background parser/tree/cache state for one document/grammar/version stream.

## Expected improvement

These are complexity reductions, not promises until measured:

| Area | Current | Target |
| --- | --- | --- |
| UTF position map after edit | O(document) + copied line strings; measured 18-26 ms at 1 MiB | O(changed lines log lines); initial build once |
| Frontend document copies | Three logical current copies before history | One current CodeMirror `Text` plus bounded detached snapshot only when needed |
| 50 MiB chunk requests | Two frontend request streams | One |
| Programmatic load history | Potential one entry per chunk | No history entries |
| Decoration install | N effects, N map clones, full retained reprojection | One patch, one range update, bounded viewport retention |
| Viewport completion | Heuristic event/timer | Explicit request ID and empty/success completion |
| Edit server tail | Ack then parse snapshot/schedule before next receive | Ack then nonblocking metadata enqueue |
| Native parser execution | Synchronous on Tokio, grammar-global mutex | Bounded blocking executor, per-document latest-wins session |
| Large viewport tasks | Up to 24 unconstrained tasks | One active per document/grammar, bounded queued latest viewport |

At minimum, removing `textIndex` rebuild recovers the measured 18-26 ms per 1 MiB edit on this host. Removing duplicate sessions halves document chunk requests. Remaining gains require real WebKit measurement.

## Performance targets and device strategy

### Device classes

1. **Development reference:** current Ryzen 9 PRO 7940HS / 61 GiB / WebKitGTK 2.52.5.
2. **Required minimum Linux device:** physical 4-core low-power x86_64 or comparable ARM64, 8 GiB RAM, integrated GPU, 1080p. This becomes release performance gate.
3. **Degraded renderer pass:** minimum device or reference host with GPU acceleration disabled/software compositing. Functional fallback must remain responsive enough to edit; do not optimize only for this mode.
4. **CPU-throttled browser fixture:** 4x slowdown for rapid CI triage. Useful signal, not substitute for Tauri/WebKit device.

### Fixture matrix

Run each relevant path against:

- sizes: 64 KiB, 1 MiB, 10 MiB, 50 MiB;
- shapes: mixed Unicode, many short lines, long lines, newline-heavy;
- modes: plain text, Rust, TypeScript/TSX, JavaScript, Markdown;
- states: one pane, four panes, inactive/remount, theme/typography update;
- actions: open, first text, ready, type burst, edit at top/middle/end, fling scroll, jump scroll, fold, diagnostics, save, reload, resync.

### Proposed targets

| Metric | Reference target | Minimum-device target |
| --- | ---: | ---: |
| Keystroke to CodeMirror local update p95 | <= 4 ms | <= 8 ms; hard max 16 ms |
| Main-thread long tasks >50 ms during 5 s typing/scroll | 0 | 0 |
| Scroll frame work p95 | <= 8 ms | <= 16 ms |
| Viewport change to current syntax patch p95, <=1 MiB | <= 50 ms | <= 100 ms |
| Viewport change to syntax patch p95, 10-50 MiB | <= 100 ms | <= 200 ms, plain-text fallback allowed while pending |
| Open to first text, <=1 MiB warm runtime | <= 100 ms | <= 200 ms |
| 50 MiB head to ready | <= 1 s | <= 2 s |
| React commits per keystroke | 0 for editor tree | 0 for editor tree |
| CodeMirror compartment reconfigures per normal edit | 0 | 0 |
| Active syntax jobs per document/grammar | <= 1 | <= 1 |
| Frontend retained syntax | visible + bounded overscan | same |

Promote wall-clock targets to blocking only after three stable runs on designated minimum hardware. Operation-count, queue-bound, cache-bound, no-history, one-session, and no-hot-path-scan invariants can block CI immediately.

## Architecture options considered

### Option A: Optimize current server Tree-sitter path - recommended first

**Benefits**

- Preserves package grammar provenance and existing trust model.
- No parser/WASM/package code in webview.
- Smallest architectural change that removes proven bottlenecks.
- Server continues sharing syntax with headless/remote clients.

**Costs**

- Syntax still has local IPC latency.
- Requires coherent patch protocol and scheduler work.

### Option B: CodeMirror/Lezer syntax for bundled first-party languages

**Benefits**

- Native CodeMirror incremental parsing/highlighting.
- No viewport syntax round trip.
- Mature viewport-aware editor integration.

**Costs**

- Adds second syntax implementation and language dependencies.
- Diverges from package-provided Tree-sitter semantics/queries.
- Third-party grammar extension and trust story must be redesigned.
- Server/headless clients still need syntax or lose parity.

**Decision:** do not adopt yet. Run a bounded spike only if Option A misses targets after P0/P1 fixes.

### Option C: First-party Tree-sitter in dedicated frontend worker

**Benefits**

- Preserves grammar family while removing server round trip.
- Main thread remains parser-free.

**Costs**

- WASM/native artifact packaging, worker protocol, duplicate trees, integrity policy, and package authority complexity.
- Harder than Option B and still duplicates server analysis.

**Decision:** no current need.

## Execution order

1. Add correlated browser/Tauri/server measurements and real hot-path regression tests.
2. Remove global duplicate session; fix one-document ownership and no-history loading.
3. Replace immutable WeakMap position index with incremental CodeMirror state.
4. Replace N decoration effects/history replay with one bounded atomic patch state.
5. Add explicit viewport request/patch IDs; fix Tauri coalescing correctness.
6. Move open/edit advisory work off connection loop; add bounded per-document syntax sessions on blocking executor.
7. Stop same-version fold recomputation; make diagnostics/folds interval-indexed.
8. Re-run full device/file/language matrix.
9. Only if targets still fail, seek approval for local parser spike and record new architecture decision.

## Decisions requiring user approval before logging

This review recommends two project decisions but does not record them yet:

1. Adopt the server-side per-document syntax session + atomic viewport patch architecture as replacement for current fragmented request/batch path.
2. Keep client-local parsing deferred behind a measured post-optimization decision gate.

Per project policy, create decision logs only after explicit user approval.

## Strengths to preserve

- Canonical server Crop rope, versions, leases, and file/workspace authority.
- CodeMirror-local optimistic typing.
- Bounded file read, chunk transfer, resident memory, frame size, and atomic streaming save from Plan 098.
- Typed inert decoration/diagnostic/fold data.
- Package provenance and two runtime trust domains.
- No package JavaScript in browser paint/layout/input paths.
- Linux-first blocking validation.

## Review limitations

- No production WebKit performance trace was available; frontend measurements used jsdom and prove algorithmic cost, not final paint timing.
- Plan 098 working tree is large and uncommitted. Findings describe current files exactly and must be rebased after that work is committed.
- Windows/macOS performance was not evaluated; Linux is required platform.
- Parser wall-clock for representative 10/50 MiB real files was not rerun because current architecture lacks safe correlated frontend/server trace. Instrumentation is first implementation task.
