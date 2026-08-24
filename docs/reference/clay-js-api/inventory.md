# Clay JS API Current Functionality Inventory

This inventory classifies the current editor, protocol, behavior, key binding, configuration, package UI, and application functionality. The machine-readable source is `docs/reference/clay-js-api/api-inventory.toml`.

## Phase boundary

- Public user/programmatic surfaces are planned Clay JS facade APIs, not raw Rust functions and not raw `Deno.core.ops.op_*` functions.
- Current ordinary typing, newline handling, deletion, cursor movement, selection, scrolling, resize/viewport updates, and paint remain Rust-native client work.
- Server-owned document mutation, leases, versions, and region locks remain authoritative on the Rust server.
- Behavior manifests are inert data owned by the server and executed locally by the client for predictable hot-path behavior.
- Current Clay does not execute arbitrary JavaScript in the Rust client, grant filesystem/network/shell/workspace/package/AI authority by default, expose client/native handles to packages, or let raw `Deno.core.ops.op_*` names become the user-facing API.

## Runtime path classes

| Runtime path | Meaning | Current examples |
| --- | --- | --- |
| `client-local-hot-path` | Native client state update with no IPC or JavaScript in the input path. | Cursor movement, selection, scrolling. |
| `client-first-predictable-hot-path-and-server-ack` | The client applies predictable manifest-declared behavior immediately and queues an asynchronous server edit transaction. | Ordinary typed characters, Enter, Backspace/Delete. |
| `client-local-layout-paint` | Local viewport/layout/paint behavior, including resize-derived visible-line calculations. | Visible extraction and viewport line count. |
| `server-first-op-wrapper` | Future Clay JS facade calls a server-side op before mutating canonical document state. | Programmatic text insertion. |
| `server-first-query` | Future Clay JS facade queries server-owned document/lease state. | Document snapshots and lease queries. |
| `server-first-op-wrapper-runtime` | Runtime-backed Clay JS facade validates a server-side package/programmatic declaration through an explicit op wrapper. | `ui.serverRegisterPanelContribution`, `ui.serverRegisterComponentContribution`, `ui.serverRegisterTransientOverlayContribution`, `ui.serverRegisterInputContribution`, `ui.serverRegisterUiStateScope`, `ui.serverRegisterThemeToken`. |
| `server-side-configuration-to-behavior-manifest` | Future `~/.config/clay/init.js` configuration updates manifest/key binding metadata on the server side. | `bindKey`, `unbindKey`. |
| `background-query` | Help/agent/configuration inspection that must not block editing. | Behavior route and manifest queries. |
| `client-local-application-action` | Native application lifecycle action. | Escape/quit. |
| `client-paint-layout-hot-path` and `local-ipc-codec` | Internal implementation details excluded from public registry generation. | Webview layout/paint and protocol DTOs. |

## Public/planned classifications

| Category | Planned public API IDs | Authority | Hot-path note |
| --- | --- | --- | --- |
| Text insertion | `editor.serverInsertText` | Server-authoritative document mutation | Typed characters remain client-first predictable and async to the server; the API is the future programmatic authoritative mutation path. |
| Newline | `editor.serverInsertNewline` | Server-authoritative mutation with behavior context | Enter uses inert manifest rules locally for indentation/comment continuation, then emits an async edit. |
| Backspace/Delete | `editor.serverDeleteRange` | Server-authoritative mutation | Local delete behavior emits async server transactions when the manifest allows deletion/replacement. |
| Cursor movement | `editor.clientMoveCursor` | Client-local UI state | Arrow/Home/End movement is local and does not serialize document text. |
| Selection | `editor.clientSetSelection` | Client-local UI state | Shift-arrow and pointer drag are transient local state. |
| Scrolling | `editor.clientScrollTo` | Client-local UI state | Wheel/page/line scrolling updates viewport/visual overflow locally. |
| Resize/viewport | `editor.clientSetViewport` | Client-local UI state | Resize changes bounded visible-line extraction, not full-document IPC. |
| Cursor style/customization | `editor.clientSetCursorStyle` | Configuration-driven client UI state | Planned configuration metadata affects paint-time UI only. |
| Editor transforms/folding/inlay visibility | `editor.toggleComment`, `editor.toggleListMarker`, `editor.rotateHeading`, `editor.clientToggleFold`, `editor.toggleInlayHints` | Client-local command IDs | These helpers name bindable commands; manifest-driven line transforms, fold collapse, and inlay visibility stay native/client-local with no JavaScript on the keypress-to-paint path. |
| Key binding management | `keybindings.bindKey`, `keybindings.unbindKey`, `keybindings.listKeyBindings` | Configuration API | Future configuration produces inert manifests; keypresses do not run JavaScript. |
| Behavior manifest routing | `behavior.getActiveBehaviorManifest`, `behavior.listBehaviorRoutes` | Server-owned behavior query | Query/inspection only; local route decisions use installed manifests. |
| Slot-aware package UI contribution | `ui.serverRegisterPanelContribution`, `ui.serverRegisterComponentContribution`, `ui.serverRegisterTransientOverlayContribution`, `ui.serverRegisterInputContribution`, `ui.serverRegisterUiStateScope`, `ui.serverRegisterThemeToken` | Server-validated package UI declaration | Runtime-backed public APIs validate package-prefixed inert panels, component trees, overlays, input/focus/action metadata, UI state-scope lifecycle metadata, and typed theme tokens at package load/config/update time; they are not client hot-path work and now have per-API Markdown docs and generated registry coverage. |
| Lease/read-only state | `documents.serverGetDocumentSnapshot`, `documents.serverGetDocumentLease` | Server-owned document/lease state | Explicit queries outside paint/input; editing is lease-validated server-side. |
| Escape/quit/application actions | `application.quit` | Client application lifecycle | Escape currently submits a native action without IPC/JavaScript. |

## Internal-only exclusions

The inventory also records implementation details that must not be included in public registry generation:

- `internal.editor.buffer`: local rope mutation and visible extraction behind editor APIs.
- `internal.editor.layoutPaint`: webview (React/CodeMirror) layout and paint internals.
- `internal.protocol.dto`: protocol serialization DTOs and local IPC codec contracts.

Plan 034 runtime hardening does not add a public Clay JS API. `runtime.timeout` and `runtime.heap_limit` are diagnostic codes, not facade IDs. `src/server/runtime_sandbox.rs`, `src/bin/clay-runtime-sandbox.rs`, sandbox protocol frames, child-process lifecycle controls, payload budgets, timeout kill/restart policy, and `RuntimeSandboxSupervisor` are internal `#[doc(hidden)]` test/harness surfaces. They must not appear in `docs/index.md`, `docs/reference/clay-js-api/api-inventory.toml`, generated registry data, runtime JS facade modules, or user-facing `Deno.core.ops` calls.

These records exist so validation and future audits can distinguish intentional public API candidates from implementation details.

## Security summary

Every public/planned entry records permissions and a negative authority statement. The current inventory grants no filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw-op, native-widget, raw-CSS, renderer-callback, or client-side JavaScript execution authority. Runtime-backed entries must use curated facade modules and explicit `deno_core` op wrappers rather than exposing raw op names.
