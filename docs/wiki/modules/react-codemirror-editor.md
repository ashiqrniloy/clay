# React CodeMirror Editor and Optimistic Document Sync

## Source

- `frontend/src/editor/{ClayEditor,create-editor,transactions,compartments,position-index,position-map,performance}.ts*`
- `frontend/src/editor/extensions/{controller,behavior,keymaps,render-patch,decorations,diagnostics,completion,folding,intelligence,accessibility}.ts`
- `frontend/src/editor/sync/{session,operations,messages}.ts`
- `frontend/src/state/document-store.ts`
- `src/editor/position_map.rs`
- `src-tauri/src/bridge/editor.rs`
- `frontend/src/editor/{performance.test.ts,position-map.test.ts}`
- `frontend/src/editor/extensions/{controller,extensions,performance,render-patch}.test.ts`
- `frontend/src/editor/**/*.test.ts*`
- `frontend/src/test/editor.test.tsx`

## Overview

Plan 097 Phase 5 ports the native optimistic document client onto CodeMirror 6;
Phase 7 projects the complete validated editor/intelligence event families.
CodeMirror owns live shadow text, selections, history, folds, and viewport-local
render state. React owns document chrome. Existing Rust queues, parsers,
providers, LSP bridges, validation, provenance, and versions remain authority.
Plan 099 adds an opt-in source-free performance trace beside this path; it
observes browser/CodeMirror/bridge/server milestones without moving authority.
Plan 099 also collapses ownership: there is no app-wide document session
singleton — `use-clay-session.ts` routes every document/tab envelope through
the workspace controller to the owning pane session, and each pane session
keeps exactly one current `Text` (the attached `view.state.doc`, or a detached
snapshot while no view exists).

## Responsibilities

- Mount one `EditorView` per visible document identity. `drawSelection()` owns the caret (`.cm-cursor`); native WebKitGTK caret is painted in the surface color so backspace cannot leave a ghost bar at the previous offset.
- Own exactly one current `Text` per pane session: `view.state.doc` while
  attached; a detached snapshot (latest user text, acked or not) only while no
  view exists. `installAuthoritative` compares by content (`Text.eq`), so a
  same-length reload/resync with changed content still installs.
- Apply user transactions locally, then enqueue UTF-8 byte-range deltas.
- All server-authored installs (head, chunk appends, resync, reload, close)
  dispatch through `programmaticAnnotations()` — `clayOrigin`
  `"programmatic"` plus `Transaction.addToHistory.of(false)` — so progressive
  loads can never be undone into partial chunks.
- Project ack / reject / resync / open / save / reload / close onto metadata
  and, when required, replace the CodeMirror document.
- Convert UTF-16 editor offsets to UTF-8 protocol offsets through the
  shared incremental `bytePositionField` StateField (`position-index.ts`):
  a persistent order-statistic treap over 64-line chunks carrying only
  numeric line widths (no line-string copies — history states share
  structure). Initial build is one O(document) pass; each edit replaces
  only its touched lines plus an O(log lines) path; conversion is one tree
  descent plus an intra-line scan read straight from the immutable
  document. Edits, viewport requests, decorations, diagnostics, folds,
  completion, intelligence, and selections all read the same field via
  `positionIndex(state)`; edit emission passes the transaction start
  state's index so the keystroke path never rebuilds.

Non-responsibility: language parsing/provider execution, LSP transport, package
JavaScript, package SDUI/Markdown preview panels (Phase 8), panes, and tabs.
Behavior manifests and intelligence results remain server-issued inert data.

## How It Works

1. Bootstrap `installInitial` paints the bounded head (first ≤ 256 KiB) and
   records metadata, then asks `GetDocumentStatus` so `workspaceRootId` /
   path can land. Heads smaller than `totalBytes` enter progressive chunk
   assembly: one outstanding versioned `documentChunkRequest` at a time,
   each continuing from the received end, appended as annotated
   transactions; editing stays read-only with a visible loading status until
   the document is complete (Plan 098). Syntax requests are not gated by this
   edit lock: the authoritative loaded head requests its visible viewport
   immediately while later chunks continue appending.
2. `ClayEditor` creates the view once per `documentId` directly from the
   rope-backed snapshot (`Text`, never a flattened string). Theme, keymap,
   language, behavior, decorations, and read-only live in `Compartment`s.
3. `EditorView.updateListener` emits only `user` / `undo` transactions.
   `resync` / `programmatic` / `remote` / `correction` annotations are silent.
4. `session_request` JSON `family: "edit"` is stamped and queued by the
   bridge. Queue-full / send failure does not roll back already-applied text.
5. `editAck` advances confirmed version. Recoverable `editRejected` reasons
   send `requestResync`. `resyncSnapshot` / `documentOpened` /
   `documentReloaded` replace the view and clear pending. Detaching a view
   stashes `view.state.doc` as the detached snapshot; a remounted pane
   restores the latest user text from it without a live duplicate copy.
   During multi-pane restore, each session retains its in-flight open path;
   `workspace-controller.ts` routes an unclaimed `documentOpened` reply by
   that path before falling back to document id, preventing placeholder-id
   collisions and cross-pane document replacement.
6. Save / reload / close / open are existing `ClientMessage` families.
7. `EditorProjection` consumes routed validated events by exact document/version.
   Its viewport request/reply state machine is documented in
   [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md).
   It keeps request orchestration only — no retained feature caches, no
   attach-time replay. Render data is owned by CodeMirror state fields
   driven by one generic atomic patch effect (`extensions/render-patch.ts`):
   - a decoration patch carries already-projected UTF-16 mark/inlay/link
     items plus the covered viewport range; the decoration field replaces
     exactly the same-authority items intersecting that covered range,
     drops marks intersected by local deletions, clears all decoration state
     synchronously when the document becomes empty, and prunes outside covered
     ± max(4,096, covered) so retained render data is visible plus bounded
     overscan. An authoritative `ViewportRenderPatch` with `status: empty`
     also clears syntax decoration state instead of treating zero output as a
     no-op;
   - diagnostics live in their own state field with covered-range
     replacement, edit mapping, and suppressor-interval merge + binary-search
     suppression (no quadratic scan); the lint extension is synced with
     `setDiagnostics` in the same transaction;
   - folds live in a sorted field with whole-set per-package replacement and
     binary-search lookup per visible line;
   - async completion and intelligence requests with cancellation/timeouts;
   - server textobject/smart-select ranges applied to multi-selections.
8. Visible-range changes use the protocol v29 atomic viewport render
   protocol: each request carries a monotonic request id, and the server
   answers with exactly one `ViewportRenderPatch` (complete, empty, or
   rejected) whose reply - not a timer - frees the request pipe; stale
   request ids drop on arrival. The 400 ms heuristic safety valve is gone.
   No document snapshot crosses for scrolling. Each profiled request carries
   one numeric trace ID through the bridge, server receive, syntax
   queue/start/end, patch delivery, and client patch/paint-adjacent markers.
   The client sends the first on-screen fragment only, clamped to 64 KiB
   chars (not the min/max of line-gap fragments, which is 0..doc.length on
   a long line). `view.inView` is not a gate: WebKitGTK's first measure can
   report a 0-height pixel viewport. Server-side, one request schedules one
   rope-sliced window (`Document::parse_windows_covering`, cap 1) - the
   native handler parses a single window, and a per-document session would
   drop extra windows that share the request id. Empty windows still send
   an empty patch so the client pipe cannot stall. Further windows arrive
   as the user scrolls.
9. Rust resolves all 37 editor vocabulary/layer styles into the theme snapshot.
   CSS variables carry color/background/attributes/scale; package CSS does not.
10. Inactive pane sessions retain at most 256 validated feature events for lazy
    editor-chunk replay. This is the documented ceiling; use a chunk-keyed LRU
    only if measured inactive-pane churn exceeds it.

```ts
view.dispatch({
  changes: { from, to, insert },
  annotations: clayOrigin.of("user"),
});
```

## Primitive Coverage

- Documents / edits: reuse server ropes, versions, leases; adapter is the
  existing bridge queue; projection is CodeMirror + `document-store`.
- Position map: generic conversion, no authority. The index stores only
  numeric line widths — never line text — so retained history states cannot
  duplicate document strings.
- Atomic render patch: `extensions/render-patch.ts` is the shared
  `applyRenderPatch` effect. Decoration, diagnostic, and fold fields replace
  only their covered authority and retain visible plus bounded overscan state.
  The bridge/controller owns request orchestration, not projected render data.

## Invariants and Constraints

- No React state holds document text. Chrome rerenders on metadata only.
- One current `Text` per session, never a second authoritative copy beside
  the live view.
- Read-only reconfigures fire only when the derived boolean
  (`!editable(access) || loading`) flips; per-keystroke metadata updates are
  inert. Workspace shell notifications expose loading/diagnostic transitions
  through `shellStatusProjection`, while layout persistence fires only on
  document identity/path/dirty transitions (`persistenceKeyProjection`);
  version/pending churn stays pane-local.
- `WorkspaceController.notify()` publishes a fresh tab snapshot identity so
  `useSyncExternalStore` observes transient status changes without pretending
  tab data or persistence keys changed.
- CodeMirror mounts its base/theme CSS through runtime `<style>` elements. The
  desktop CSP therefore permits `'unsafe-inline'` for `style-src` only;
  `script-src` remains `'self'` with inline/eval execution forbidden. Without
  this scoped exception, WebKitGTK rejects CodeMirror's sheets and stacks the
  gutter above the content, leaving loaded text outside the clipped viewport.
- No await on the keystroke path. `send` is fire-and-forget.
- Ordinary edits are deltas. Full text only on initial/open/reload/resync.
- Frontend cannot mint leases or skip server version checks. Tauri overwrites
  forged nested completion/intelligence/selection `clientId` values.
- IDs stay JSON numbers (sequential). Offsets convert at the one map.
- `PerformanceRecorder` is disabled by default, retains at most 4096 events,
  sanitizes feature names, and stores no source/path/secret fields. Typing
  reuses its transaction ID as the trace ID; viewport `traceId` is optional
  protocol metadata.
- Link activation accepts only same-document ranges or safe relative paths;
  URLs, absolute paths, fragments, and traversal never invoke navigation.
- Hover Markdown becomes inert `textContent`; code-action edits remain preview
  only; inlays are CSS-generated and absent from editable accessibility text.
- `@clay/markdown` preview remains existing bounded package SDUI by prior
  architecture decision. Phase 7 ports Markdown editor rendering; Phase 8
  projects that package panel. No duplicate browser Markdown/HTML authority.

## Tests

- `src/editor/position_map.rs` and `frontend/src/editor/position-map.test.ts`:
  shared golden vectors, mid-surrogate snap, TextEncoder agreement, and
  differential property tests proving the incremental treap equals both the
  linear reference and a fresh rebuild after every edit of random
  multi-chunk sequences.
- `frontend/src/editor/sync/{operations,session}.test.ts`: change mapping,
  ordered ack, stale reject → resync, open/save/close, blocked send,
  50 MiB-head single-request-per-offset, duplicate-chunk dedupe,
  same-length reload install, no-history chunk installs, detach/remount
  latest-user-text restore.
- `frontend/src/test/editor.test.tsx`: compartment reconfigure keeps text;
  local dispatch before blocked IPC; read-only reconfigure count stays flat
  across unrelated metadata updates.
- `frontend/src/editor/extensions/extensions.test.ts`: syntax/link/inlay,
  folding, manifest transforms, and multi-selection.
- `frontend/src/editor/extensions/render-patch.test.ts`: covered-range
  replacement, authority isolation, edit mapping, and bounded retention.
- `frontend/src/editor/extensions/controller.test.ts`: stale-version denial,
  visible byte range request, gated textobject round trip.
- `frontend/src/editor/performance.test.ts`: disabled behavior, bounded drops,
  source safety, percentiles, and end-to-end trace-stage ordering.
- `frontend/src/editor/extensions/completion.test.ts`: request/result round trip,
  LSP bare-tabstop conversion, local snippet expansion and placeholder selection.
- `frontend/src/editor/extensions/performance.test.ts`: repeated 1 MiB
  typing on the real Clay edit path, constant-size viewport retention, 50 MiB
  one-Text ownership, four-pane isolation, and software-render smoke. Wall
  clock is advisory; work/ownership invariants are blocking.
- Existing Rust `completion_provider`, `language_intelligence`,
  `decoration_transport`, `syntax_grammar`, and editor hot-path suites remain
  the provider/validation authority.
- `src-tauri/src/bridge/editor.rs`: camelCase document event shape.
- `src-tauri/tests/config_security.rs`: CodeMirror-compatible style CSP while
  keeping script inline/eval and remote origins forbidden.

```bash
cargo test -p clay --lib editor::position_map
cargo test -p clay-desktop --lib bridge::editor
cd frontend && npm test
```

## Related

- [Versioned Text Synchronization](../flows/versioned-text-synchronization.md)
- [Editor Viewport Render Patch](../flows/editor-viewport-render-patch.md)
- [Client Edit Emission](../flows/client-edit-emission.md)
- [Desktop Typed Bridge](desktop-typed-bridge.md)
- [React Shell](react-shell.md)
- `docs/development/tauri-react-primitive-migration.md`
