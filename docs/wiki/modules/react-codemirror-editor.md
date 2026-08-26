# React CodeMirror Editor and Optimistic Document Sync

## Source

- `frontend/src/editor/{ClayEditor,create-editor,transactions,compartments,position-map}.ts*`
- `frontend/src/editor/extensions/{controller,behavior,keymaps,decorations,diagnostics,completion,folding,intelligence,accessibility}.ts`
- `frontend/src/editor/sync/{session,operations,messages}.ts`
- `frontend/src/state/document-store.ts`
- `src/editor/position_map.rs`
- `src-tauri/src/bridge/editor.rs`
- `frontend/src/editor/**/*.test.ts*`
- `frontend/src/test/editor.test.tsx`

## Overview

Plan 097 Phase 5 ports the native optimistic document client onto CodeMirror 6;
Phase 7 projects the complete validated editor/intelligence event families.
CodeMirror owns live shadow text, selections, history, folds, and viewport-local
render state. React owns document chrome. Existing Rust queues, parsers,
providers, LSP bridges, validation, provenance, and versions remain authority.

## Responsibilities

- Mount one `EditorView` per visible document identity.
- Apply user transactions locally, then enqueue UTF-8 byte-range deltas.
- Project ack / reject / resync / open / save / reload / close onto metadata
  and, when required, replace the CodeMirror document.
- Convert UTF-16 editor offsets to UTF-8 protocol offsets through the
  memoized per-document line index (`textIndex` in `position-map.ts`):
  O(log lines + line length) per conversion, never a full-document scan or
  flattened string. The index is keyed on the immutable CodeMirror `Text`, so
  it rebuilds once per document version and is shared by edit emission,
  viewport requests, decoration/diagnostic/fold projection, completion, and
  intelligence.

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
   the document is complete (Plan 098).
2. `ClayEditor` creates the view once per `documentId` directly from the
   rope-backed snapshot (`Text`, never a flattened string). Theme, keymap,
   language, behavior, decorations, and read-only live in `Compartment`s.
3. `EditorView.updateListener` emits only `user` / `undo` transactions.
   `resync` / `programmatic` / `remote` / `correction` annotations are silent.
4. `session_request` JSON `family: "edit"` is stamped and queued by the
   bridge. Queue-full / send failure does not roll back already-applied text.
5. `editAck` advances confirmed version. Recoverable `editRejected` reasons
   send `requestResync`. `resyncSnapshot` / `documentOpened` /
   `documentReloaded` replace the view and clear pending.
6. Save / reload / close / open are existing `ClientMessage` families.
7. `EditorProjection` consumes routed validated events by exact document/version:
   - chunk-keyed syntax/semantic/link/inlay marks; provisional CM range mapping;
   - source-keyed lint diagnostics and provenance-keyed fold services;
   - async completion and intelligence requests with cancellation/timeouts;
   - server textobject/smart-select ranges applied to multi-selections.
8. Visible-range changes use inflight pacing: the first request for a new
   viewport sends immediately (highlight latency ≈ one round trip); while one
   is on the wire, newer viewports collapse latest-wins and fire the moment a
   decoration/diagnostic/fold reply lands (400 ms safety valve covers lost
   replies; detach/clear cancels pending work). No document snapshot crosses
   for scrolling. Server-side, one request schedules rope-sliced windows
   covering the WHOLE requested viewport (`Document::parse_windows_covering`,
   capped at 24 windows) — no full-text clone, no prefix rescan, and tall or
   zoomed-out viewports are fully highlighted instead of clamped to the first
   parse window.
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
- Position map: generic conversion, no authority.

## Invariants and Constraints

- No React state holds document text. Chrome rerenders on metadata only.
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
- Link activation accepts only same-document ranges or safe relative paths;
  URLs, absolute paths, fragments, and traversal never invoke navigation.
- Hover Markdown becomes inert `textContent`; code-action edits remain preview
  only; inlays are CSS-generated and absent from editable accessibility text.
- `@clay/markdown` preview remains existing bounded package SDUI by prior
  architecture decision. Phase 7 ports Markdown editor rendering; Phase 8
  projects that package panel. No duplicate browser Markdown/HTML authority.

## Tests

- `src/editor/position_map.rs` and `frontend/src/editor/position-map.test.ts`:
  shared golden vectors, mid-surrogate snap, TextEncoder agreement.
- `frontend/src/editor/sync/{operations,session}.test.ts`: change mapping,
  ordered ack, stale reject → resync, open/save/close, blocked send.
- `frontend/src/test/editor.test.tsx`: compartment reconfigure keeps text;
  local dispatch before blocked IPC.
- `frontend/src/editor/extensions/extensions.test.ts`: syntax/link/inlay,
  folding, manifest transforms, and multi-selection.
- `frontend/src/editor/extensions/controller.test.ts`: stale-version denial,
  visible byte range request, gated textobject round trip.
- `frontend/src/editor/extensions/completion.test.ts`: request/result round trip,
  LSP bare-tabstop conversion, local snippet expansion and placeholder selection.
- `frontend/src/editor/extensions/performance.test.ts`: 1 MiB local typing and
  1,000-span viewport budgets.
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
- [Client Edit Emission](../flows/client-edit-emission.md)
- [Desktop Typed Bridge](desktop-typed-bridge.md)
- [React Shell](react-shell.md)
- `docs/development/tauri-react-primitive-migration.md`
