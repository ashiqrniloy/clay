# Phase 19 Windows File Open Primitive Review

## Source

- `plans/022-Phase19-Windows-Markdown-File-Open-Dialog-Smoke.md`
- `docs/reference/primitives/index.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/parse-update-strategy.md`
- `docs/reference/primitives/rendering-strategy.md`
- `docs/wiki/modules/primitive-architecture.md`
- `docs/wiki/flows/client-behavior-routing.md`
- `docs/wiki/flows/client-server-edit-ack.md`
- `docs/wiki/modules/client-snapshot-bootstrap.md`
- `docs/wiki/modules/server-file-workspace.md`
- `docs/wiki/modules/behavior-runtime-registration.md`
- `docs/wiki/modules/mode-registry.md`
- `docs/wiki/modules/parse-coordinator.md`
- `docs/wiki/modules/decoration-transport.md`
- `docs/wiki/modules/first-party-markdown-package.md`

## Overview

This review completes the primitive-first checkpoint before Phase 19 implementation. The manual smoke goal is Windows-specific, but the Rust primitives should not become Markdown-specific or Windows-specific except for the isolated native dialog backend. The generic flow should be:

```text
Key binding -> client UI command intent -> Windows file dialog -> selected-file IPC request -> server single-file grant/open -> document-open activation -> parse/decor/status publication.
```

The client may own the native file picker because it is explicit UI interaction. The server remains authoritative for document state, selected-path validation, single-file authorization, package JavaScript execution, mode activation, and publication of inert behavior/decorations/SDUI. Ordinary typing, paint, layout, scroll, and text-event paths must continue to use local inert state and must not wait on JavaScript, IPC, or file IO.

## Existing Primitive Inventory

| Primitive area | Current source paths | What works today | Timing classification | Permission / authority boundary |
| --- | --- | --- | --- | --- |
| Keybinding and configuration | `runtime/js/keybindings.ts`, `src/server/ops/keybindings.rs`, `src/server/behavior.rs`, `docs/wiki/modules/behavior-runtime-registration.md` | `~/.config/clay/init.js` can call `bindKey` for runtime-bindable command IDs, compile the binding into an inert `BehaviorManifest`, and publish an atomic behavior-version update. | Configuration-time/startup-time work; keypress later performs only a local manifest lookup. | Server-side configuration validates chord, scope, command allowlist, and routing policy. The manifest grants no filesystem, shell, network, package install, AI, WASM, raw-op, or client-side JavaScript authority. |
| Behavior manifests and client key routing | `src/client/behavior.rs`, `src/editor/surface.rs`, `src/masonry_editor.rs`, `docs/wiki/flows/client-behavior-routing.md` | The client can route keys to `ClientFirstPredictable` local edits or `ServerFirst` intent records without executing JavaScript or waiting for IPC before local paint. | Hot-path key routing is local and bounded; server-intent handling is explicit follow-up work outside paint. | The client owns native input/routing of installed inert declarations only. Current route kinds do not distinguish client-owned UI commands from server-first commands. |
| Client command routing and GUI event bridge | `src/masonry_editor.rs`, `src/main.rs`, `src/client/mod.rs`, `docs/wiki/flows/client-server-edit-ack.md` | Background IPC events are bridged into Masonry actions and applied on the GUI thread; SDUI, diagnostics, decorations, resync snapshots, and behavior manifests already use this non-blocking event path. | GUI action/event-loop work after an event arrives; no socket reads or writes in paint/text handlers. | Client owns native UI state. Server-sent data remains decoded/inert before widget mutation. A generic client UI command outcome is missing. |
| IPC document open messages | `src/protocol/mod.rs`, `src/protocol/codec.rs`, `src/server/connection.rs`, `docs/wiki/modules/protocol-codec.md` | `ClientMessage::OpenDocument { workspace_root_id, path }` and `ServerMessage::DocumentOpened { metadata, text }` round-trip workspace-authorized file opens through the shared bounded `rkyv` codec. | Explicit file-open command time; the response snapshot may carry full text only for the open/resync boundary, while later edits remain delta-based. | The message is server-first and workspace-root scoped. It does not model a user-selected absolute path or selected-file-only authority grant. |
| Server workspace validation | `src/server/workspace.rs`, `src/server/connection.rs`, `docs/wiki/modules/server-file-workspace.md` | The server canonicalizes workspace-root paths, rejects traversal/symlink escapes, directories, special files, invalid UTF-8, and duplicate opens, then registers a file-backed `DocumentState`. | Server file-open time only; ordinary edit application does not do file IO or workspace scans. | Server owns canonical paths, document registry, file validation, dirty state, and diagnostics. Current authorization is root-scoped, not selected-file-scoped. |
| Client snapshot/document replacement | `src/client/mod.rs`, `src/editor/surface.rs`, `src/masonry_editor.rs`, `docs/wiki/modules/client-snapshot-bootstrap.md` | Initial snapshots and `ResyncSnapshot` events replace the editor buffer, reset caret/selection/viewport/layout cache, and refresh document access/version status. | Startup/resync/document-open boundary only; ordinary edits remain local/delta-based. | Client applies server-authorized snapshots only. Ongoing `DocumentOpened` messages are currently not converted into a GUI replacement event. |
| Mode activation | `src/packages/modes.rs`, `src/server/ops/modes.rs`, `runtime/js/modes.ts`, `docs/wiki/modules/mode-registry.md` | Static package mode patterns can classify `.md`, `.markdown`, `.mdown`, and `text/markdown`, then activate one server-owned major mode and behavior version for a document. | Open/reload/configuration-time work; installed manifests are used locally after activation. | Requires `mode-registration` and `mode-activation`; the Rust client does not choose modes or run package JavaScript. |
| Parse handler registration and adapter scheduling | `src/protocol/parse.rs`, `src/server/parse_coordinator.rs`, `src/server/ops/parse.rs`, `runtime/js/parse.ts`, `docs/wiki/modules/parse-coordinator.md` | Packages can register permission-gated parse handlers, receive bounded `ParseWindowSnapshot` data, run background parse work, cancel stale generations, and publish validated updates. | Registration is configuration/load time; parse execution is background/open/edit/viewport work, never keypress/paint blocking. | Requires `parse-document`; source delivery is bounded to already-open document windows with package/mode provenance and stale-version checks. A generic document-open parse trigger must wire opened files into this path. |
| Decoration transport and native rendering | `src/protocol/decorations.rs`, `src/server/decorations.rs`, `src/server/ops/decorations.rs`, `runtime/js/decorations.ts`, `src/editor/surface.rs`, `docs/wiki/modules/decoration-transport.md` | Server-side package adapters can publish viewport/chunk-bounded inert `DecorationSet` data; the client stores validated chunks and paints known style tokens locally. | Background publication and GUI-thread application outside paint; paint consumes cached spans only. | Requires `render-decorations`; validates document version, byte ranges, viewport/chunk range, package provenance, style tokens, and `DECORATION_PAYLOAD_BUDGET_BYTES`. |
| SDUI and status | `src/protocol/sdui.rs`, `src/server/sdui.rs`, `src/masonry_sdui.rs`, `src/masonry_editor.rs`, `docs/wiki/modules/server-driven-ui.md`, `docs/wiki/modules/masonry-editor.md` | Runtime/package status and preview UI can be published as inert SDUI; runtime diagnostics and connection/access/version status are visible in GUI chrome. | Background/status update path; native paint renders already-validated state. | SDUI is inert and bounded. Diagnostics must be sanitized and must not reveal unauthorized paths, document text, secrets, shell output, or host internals. |
| Markdown package adapters | `packages/markdown/dist/load.js`, `packages/markdown/dist/parser.js`, `packages/markdown/dist/sdui.js`, `docs/wiki/modules/first-party-markdown-package.md` | The first-party package declares Markdown patterns/permissions, registers mode/commands/parser metadata, parses bounded windows with package-owned markdown-it logic, publishes generic spans, and builds inert status SDUI. | Package load/open/background parse/status work only; no package JavaScript runs in the Rust client hot path. | Package permissions are `mode-registration`, `mode-activation`, `command-registration`, `parse-document`, and `render-decorations`; parser output is generic Clay decoration data, not markdown-it tokens or HTML. |

## What Existing Primitives Can Achieve

Existing primitives already cover most of the smoke path once an authorized opened document exists:

- `bindKey` can express the user-facing `Ctrl+O` configuration pattern, but its allowlist and route type need a generic client-UI command authority.
- Behavior manifests can keep the keypress path local and inert.
- The Windows dialog can live behind a client-owned backend because it is explicit user UI, not package JavaScript or server file scanning.
- Workspace open code already validates regular UTF-8 files and creates server-canonical documents, but it only authorizes workspace-root paths.
- Initial/resync snapshot loading already knows how to replace the editor buffer and reset local state; a document-open event can reuse that boundary.
- Mode classification, Markdown package activation, parse-window scheduling, decoration transport, and SDUI status are all generic and reusable.

## Generic Primitive Gaps Before Implementation

The Phase 19 implementation should add only reusable primitives:

1. **`ClientUiCommandIntent` / client UI command route**
   - Add a manifest route for commands that must be handled by the native client before any server request exists.
   - `clay.documents.clientOpenFileDialog` should bind through `init.js` and route to this generic intent, not to a hard-coded `Ctrl+O` branch and not to a server-first command.
   - The route grants only a user-mediated native UI prompt. It must not grant filesystem scanning, package loading, shell, network, AI, WASM, raw ops, or client-side JavaScript.

2. **`SelectedFileOpenRequest` / `SelectedFileGrant`**
   - Add a protocol/server workspace primitive for an explicit user-selected path.
   - The server must canonicalize the path, require a regular UTF-8 text file, sanitize diagnostics, and authorize at most that canonical file. It must not add the parent directory as a workspace root and must not authorize sibling files.
   - This primitive should be reusable by future native pickers or drag/drop flows that carry explicit user-selected files.

3. **`DocumentOpenApplied` / opened-document client event**
   - Convert `DocumentOpened` into the same GUI-safe snapshot replacement boundary used by startup/resync: update document ID, version, text, access, lease/sync state, caret/selection/viewport/layout cache, and status.
   - Keep full-text transfer limited to initial open/resync snapshots; ordinary edits must continue through `Edit` deltas and bounded queues.

4. **`DocumentOpenActivation` / open-time mode refresh**
   - After any server-authorized document open, run generic document classification, major-mode activation, behavior manifest publication, and initial parse/decor/status refresh.
   - The activation primitive should be keyed by document metadata and loaded package contributions, not by `if extension == ".md"` or `if mode == "markdown"` branches.

5. **`ParserAdapterExecution` open trigger**
   - Reuse the existing parse coordinator and bounded parse-window snapshots to run package adapters after document open.
   - If a new helper is required, name it for generic parse-adapter execution or document-open parse scheduling. Do not add `MarkdownOpenParser`, `MarkdownFileDecorations`, markdown-it token structs, HTML preview renderers, or Rust Markdown syntax branches.

6. **`ClientFileDialogBackend` platform abstraction**
   - Isolate Windows COM dialog code behind a small client abstraction that returns `Selected(PathBuf)`, `Cancelled`, or `Unsupported`.
   - The abstraction may have a Windows backend, but the rest of the flow should consume generic selected-file results so non-Windows platforms can report unsupported diagnostics without panics.

## Rejected Markdown- or Windows-Specific Rust Work

Do not add Rust parser, protocol, workspace, client-route, or renderer branches named for Markdown syntax or the Phase 19 smoke fixture. Rejected examples include `MarkdownOpenDocument`, `MarkdownSelectedFileGrant`, `MarkdownParserOnOpen`, `MarkdownHeading`, `MarkdownFence`, `MarkdownItToken`, `if mode == "markdown"` parser/render paths, and hard-coded `Ctrl+O` in `EditorWidget`.

Windows-specific code is acceptable only inside the dialog backend/module. The command route, selected-file grant, document-open event, mode activation, parse scheduling, decoration transport, and status handling should remain platform- and mode-neutral.

## Security and Performance Classification

- **Configuration-time:** `init.js` evaluation, `bindKey`, command allowlist validation, package load/registration, fixed dialog-command documentation.
- **Explicit UI-command time:** client route handling and native file dialog. This is the only modal UI/file-picker work in scope.
- **Server file-open time:** selected-path canonicalization, regular-file and UTF-8 validation, single-file grant creation, initial snapshot transfer, typed/sanitized diagnostics.
- **Document-open/background time:** mode activation, behavior manifest publication, parse-window scheduling, package parser adapter execution, decoration and SDUI/status publication.
- **Hot-path typing/paint/text-event work:** local behavior manifest lookup, local edit application, cached decoration painting, status rendering. No JavaScript, IPC wait, file IO, dialog invocation, parser execution, or full-document serialization belongs here.

## Decision for Phase 19 Implementation

Proceed with implementation only after the generic gaps above are represented in the relevant tasks. The first implementation task should introduce the bindable client UI command route and keep the Windows dialog backend isolated. Later tasks should add selected-file open authorization, document-open application, and generic open-time activation/parse scheduling before relying on the Markdown package for visible decorations.

## Verification

This review satisfies the Phase 19 primitive gate:

- Inventory reviewed: keybinding/configuration, behavior manifests, client command routing, IPC document open messages, server workspace validation, client snapshot replacement, mode activation, parse handler registration, decoration transport, SDUI/status, and Markdown package adapters.
- Generic gaps recorded: `ClientUiCommandIntent`, `SelectedFileOpenRequest`, `SelectedFileGrant`, `DocumentOpenApplied`, `DocumentOpenActivation`, `ParserAdapterExecution`, and `ClientFileDialogBackend`.
- Security boundaries preserved: client owns native UI prompt only; server owns selected-file validation and document state; package JavaScript runs server-side only; client rendering is inert; diagnostics are sanitized.
- Hot-path boundaries preserved: typing, paint, layout, scroll, and text events continue to avoid JavaScript, IPC waits, file IO, parser execution, and modal dialogs.

## Tests

- `tests/primitives_docs.rs::phase19_file_open_primitive_review_records_existing_inventory`
- `tests/primitives_docs.rs::phase19_file_open_primitive_review_records_generic_gaps_only`
- `cargo test --test primitives_docs`

## Related

- [Primitive Architecture](primitive-architecture.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Client/Server Edit Acknowledgement Flow](../flows/client-server-edit-ack.md)
- [Server File Workspace Model](server-file-workspace.md)
- [Mode Registry](mode-registry.md)
- [Parse Coordinator](parse-coordinator.md)
- [Decoration Transport](decoration-transport.md)
- [First-Party Markdown Package](first-party-markdown-package.md)
