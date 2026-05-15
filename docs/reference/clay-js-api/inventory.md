# Clay JS API Current Functionality Inventory

This inventory classifies the current editor, protocol, behavior, key binding, configuration, and application functionality for Phase 7. The machine-readable source is `docs/reference/clay-js-api/api-inventory.toml`.

## Phase boundary

- Public user/programmatic surfaces are planned Clay JS facade APIs, not raw Rust functions and not raw `Deno.core.ops.op_*` functions.
- Current ordinary typing, newline handling, deletion, cursor movement, selection, scrolling, resize/viewport updates, and paint remain Rust-native client work.
- Server-owned document mutation, leases, versions, and region locks remain authoritative on the Rust server.
- Behavior manifests are inert data owned by the server and executed locally by the client for predictable hot-path behavior.
- Phase 7 does not execute arbitrary JavaScript in the Rust client, load user configuration, grant filesystem/network/shell/workspace/package/AI authority, or implement runtime op wiring.

## Runtime path classes

| Runtime path | Meaning | Current examples |
| --- | --- | --- |
| `client-local-hot-path` | Native client state update with no IPC or JavaScript in the input path. | Cursor movement, selection, scrolling. |
| `client-first-predictable-hot-path-and-server-ack` | The client applies predictable manifest-declared behavior immediately and queues an asynchronous server edit transaction. | Ordinary typed characters, Enter, Backspace/Delete. |
| `client-local-layout-paint` | Local viewport/layout/paint behavior, including resize-derived visible-line calculations. | Visible extraction and viewport line count. |
| `server-first-op-wrapper` | Future Clay JS facade calls a server-side op before mutating canonical document state. | Programmatic text insertion. |
| `server-first-query` | Future Clay JS facade queries server-owned document/lease state. | Document snapshots and lease queries. |
| `server-side-configuration-to-behavior-manifest` | Future `~/.config/clay/init.js` configuration updates manifest/key binding metadata on the server side. | `bindKey`, `unbindKey`. |
| `background-query` | Help/agent/configuration inspection that must not block editing. | Behavior route and manifest queries. |
| `client-local-application-action` | Native application lifecycle action. | Escape/quit. |
| `masonry-paint-layout-hot-path` and `local-ipc-codec` | Internal implementation details excluded from public registry generation. | Layout/paint and protocol DTOs. |

## Public/planned classifications

| Category | Planned public API IDs | Authority | Hot-path note |
| --- | --- | --- | --- |
| Text insertion | `clay.editor.serverInsertText` | Server-authoritative document mutation | Typed characters remain client-first predictable and async to the server; the API is the future programmatic authoritative mutation path. |
| Newline | `clay.editor.serverInsertNewline` | Server-authoritative mutation with behavior context | Enter uses inert manifest rules locally for indentation/comment continuation, then emits an async edit. |
| Backspace/Delete | `clay.editor.serverDeleteRange` | Server-authoritative mutation | Local delete behavior emits async server transactions when the manifest allows deletion/replacement. |
| Cursor movement | `clay.editor.clientMoveCursor` | Client-local UI state | Arrow/Home/End movement is local and does not serialize document text. |
| Selection | `clay.editor.clientSetSelection` | Client-local UI state | Shift-arrow and pointer drag are transient local state. |
| Scrolling | `clay.editor.clientScrollTo` | Client-local UI state | Wheel/page/line scrolling updates viewport/visual overflow locally. |
| Resize/viewport | `clay.editor.clientSetViewport` | Client-local UI state | Resize changes bounded visible-line extraction, not full-document IPC. |
| Cursor style/customization | `clay.editor.clientSetCursorStyle` | Configuration-driven client UI state | Planned configuration metadata affects paint-time UI only. |
| Key binding management | `clay.keybindings.bindKey`, `clay.keybindings.unbindKey`, `clay.keybindings.listKeyBindings` | Configuration API | Future configuration produces inert manifests; keypresses do not run JavaScript. |
| Behavior manifest routing | `clay.behavior.getActiveBehaviorManifest`, `clay.behavior.listBehaviorRoutes` | Server-owned behavior query | Query/inspection only; local route decisions use installed manifests. |
| Lease/read-only state | `clay.documents.serverGetDocumentSnapshot`, `clay.documents.serverGetDocumentLease` | Server-owned document/lease state | Explicit queries outside paint/input; editing is lease-validated server-side. |
| Escape/quit/application actions | `clay.application.quit` | Client application lifecycle | Escape currently submits a native action without IPC/JavaScript. |

## Internal-only exclusions

The inventory also records implementation details that must not be included in public registry generation:

- `internal.editor.buffer`: local rope mutation and visible extraction behind editor APIs.
- `internal.editor.layoutPaint`: Masonry/Parley/Vello layout and paint internals.
- `internal.protocol.dto`: protocol serialization DTOs and local IPC codec contracts.

These records exist so validation and future audits can distinguish intentional public API candidates from implementation details.

## Security summary

Every public/planned entry records permissions and a negative authority statement. The current inventory grants no filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript execution authority. Schema metadata is descriptive; runtime permissions and `deno_core` op wrappers remain future work.
