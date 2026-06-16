# Launch and GUI Smoke Validation

Use these command-first launch paths to validate Clay's current GUI and client/server behavior on every supported desktop platform. The normal workflow does not require copying a named pipe or Unix socket path.

## Quick Commands

From the repository root:

```bash
# Start or reuse the default local server, then open the GUI client.
cargo run

# App-managed GUI smoke run with an isolated endpoint and managed child server.
cargo run -- smoke-gui

# Runtime-backed configuration smoke: evaluates tests/fixtures/configuration/runtime-sdui/init.js
# before the GUI connects, then renders the JavaScript-published SDUI tree.
cargo run -- smoke-gui --config-fixture runtime-sdui

# Markdown mode smoke: validates the first-party package SDUI preview/status workflow.
cargo run -- smoke-gui --config-fixture markdown-mode

# Windows Markdown open-dialog smoke: loads Markdown and binds Ctrl+O through init.js.
cargo run -- smoke-gui --config-fixture windows-markdown-open

# Foreground default server, useful for watching server diagnostics.
cargo run -- server

# First default client: should receive the editable lease when available.
cargo run -- client

# Second default client: should attach as a read-only observer.
cargo run -- client
```

## Default End-User Configuration

The commands above launch the app or run dev-only smoke fixtures. The actual end-user product setup is a small `~/.config/clay/init.js` that loads Markdown defaults through the runtime-backed generic package loader and binds the Windows open-file command:

```js
import { bindKey } from "clay:keybindings";
import { loadPackage } from "clay:packages";

await loadPackage("@clay/markdown");
bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
```

This is the Markdown product baseline. It is deliberately distinct from the smoke fixtures under `tests/fixtures/configuration/`:

- **Smoke-only (dev validation, never the product path):** the `markdown-mode` and `windows-markdown-open` fixtures inline a full `markdownPackage` manifest object and manually call `serverLoadPackage`, `serverRegisterModePattern`, `serverActivateMajorMode`, `serverRegisterCommand`, `serverRegisterParseHandler`, `serverPublishDecorations`, and `publishTree`. That plumbing exists only to validate each facade deterministically. Pasting the smoke fixture manifest block into `~/.config/clay/init.js` is not supported and is not the documented setup.
- **End-user (product baseline):** the one-line `loadPackage("@clay/markdown")` plus the explicit `Ctrl+O` `bindKey`. No inline manifest object, no per-facade registration imports, no `publishTree` panel publication.

Markdown end-user baseline invariants:

- **Editor-only main slot.** The Markdown editor occupies the mandatory `main` slot of `PaneSlotLayout`. No default `PanelContribution` (side panel, preview panel, or status panel) is published on load.
- **Optional preview only on demand.** An optional Markdown preview/status panel is a `clay:ui` `PanelContribution` targeting a slot such as `right` with `defaultVisibility: "hidden"`; it appears only through `setPackageOption`, `serverSetLayoutOverride`, or `markdown.togglePreview`.
- **Selected-file open is edit-only.** `Ctrl+O` opens a selected file and activates Markdown behavior/decorations through generic `MajorModeActivation` + `DocumentClassification`. Saving a file picked through the dialog is out of scope until a later phase; close or discard the smoke document after editing.

Timing and authority boundaries for the baseline:

- **Configuration/open time only.** Markdown loading, contribution-descriptor validation, and selected-file activation run at configuration load or document-open time. Ordinary typing, paint, scroll, layout, and text-event handling stay client-local/non-blocking and read only already-installed inert shell/contribution state; they never run package JavaScript, parser work, IPC, file IO, or full-document serialization.
- **No authority broadened.** Simplifying `init.js` to the one-line loader plus `bindKey` does not grant package installation, filesystem access beyond the selected file and the config root, workspace expansion, shell, network, AI mutation, WASM, raw Deno op, native-widget handle, raw CSS, renderer-callback, or client-side JavaScript authority. Package loading remains constrained to first-party `@clay/*` specifiers and deny-by-default for arbitrary external imports.

## Expected GUI Status

The GUI status line and accessibility label should make the connection state visible without reading stderr:

- `Connected — Editable`: the client has the editable document lease.
- `Connected — Read-only Observer`: the client is attached but cannot edit because another client owns the editable lease.
- `Local Fallback`: no server was reachable for `cargo run -- client`, so the GUI opened with local-only state.
- `Disconnected`: the connection was lost after a connected session.
- Version text such as `v12`: the latest known server-confirmed document version after a snapshot, resync, or edit acknowledgement.
- `Runtime clay.runtime.<code>: <safe message>`: server-side JavaScript configuration/runtime diagnostics reached the GUI status path. The message should be actionable but must not include absolute local paths, source snippets, secrets, tokens, or environment dumps.

Typing remains local and optimistic. Editor input must not wait for IPC acknowledgements, server work, runtime diagnostics, or full-document synchronization; acknowledgements, resyncs, and runtime diagnostic status updates arrive asynchronously and update status when available.

The server-driven UI smoke surface should show more than one native region when connected: a server-generated workspace/sidebar panel with status/list/button content plus the document-bound editor view. Updating or interacting with side-panel controls must not replace the editor text, caret, document version, editable/read-only status, or runtime diagnostic status text.

SDUI payload costs are validated by unit tests rather than GUI smoke output. The representative initial SDUI snapshot is expected to stay under 4 KiB, and a simple side-panel update is expected to stay under 1 KiB and below the equivalent snapshot size.

## Smoke Modes

### Bare `cargo run`

Bare `cargo run` tries the platform default local endpoint. If no server is reachable, Clay starts the current executable directly as a background `clay server <endpoint>` process, retries the client handshake for a bounded readiness window, and opens the GUI when connected.

### `cargo run -- smoke-gui`

`smoke-gui` is the isolated app-managed GUI smoke path. It creates a unique local endpoint, starts a managed child `clay server <endpoint>` process with direct arguments, waits for readiness, opens the GUI client, and terminates the child server when the GUI exits.

### `cargo run -- smoke-gui --config-fixture runtime-sdui`

The runtime-backed smoke path uses the same managed local IPC lifecycle, but passes `--config-fixture runtime-sdui` to the child server. The server evaluates `tests/fixtures/configuration/runtime-sdui/init.js`, imports `clay:sdui`, publishes a validated SDUI tree, and then sends that tree through the normal bootstrap `SduiSnapshot` path. The GUI should show the `Runtime Smoke Workspace` panel, list/button/status content, and the document-bound editor view while retaining editable/read-only connection status and asynchronous edit acknowledgements.

### `cargo run -- smoke-gui --config-fixture markdown-mode`

The Markdown smoke path uses `tests/fixtures/configuration/markdown-mode/init.js`. The fixture validates and loads `@clay/markdown` metadata, activates the `markdown` mode for `sample.md`/document `1`, registers package commands and parse/decorations providers, publishes representative decorations, and sends an inert `Markdown Preview` SDUI panel with parse/decorations status and a `Toggle Preview` button targeting `markdown.togglePreview`. If no workspace root is configured, the fixture still uses document `1` so the GUI smoke remains deterministic and does not expand filesystem authority.

Expected visible SDUI text includes `Markdown Preview`, `Mode: markdown`, `Parse: markdown-it registered`, `Decorations: published`, and `Preview: decorated editor`. Ordinary typing remains local; preview/status publication is configuration/load-time work, not keypress, paint, or scroll work.

### `cargo run -- smoke-gui --config-fixture windows-markdown-open`

The Windows Markdown open-dialog smoke path uses `tests/fixtures/configuration/windows-markdown-open/init.js`. The fixture loads `@clay/markdown`, registers the Markdown mode/parser/decorations/status workflow, and binds `Ctrl+O` to `clay.documents.clientOpenFileDialog` through the normal `bindKey` configuration API. It does not add a Rust shortcut, install packages, fetch network resources, execute shell commands, or broaden workspace authority.

Manual Windows 11 verification:

1. Run `cargo run -- smoke-gui --config-fixture windows-markdown-open`.
2. Confirm the side panel shows `Windows Markdown Open Dialog Smoke`, `Mode: markdown`, `Parse: markdown-it registered`, `Decorations: published`, and `Open: Ctrl+O native Markdown dialog`.
3. Press `Ctrl+O`, select a regular UTF-8 `.md`, `.markdown`, or `.mdown` file in the native Windows file browser, and confirm the selected file replaces the editor buffer.
4. Confirm Markdown decorations/status are visible for the opened file. Decoration refresh may arrive asynchronously; ordinary typing should remain responsive and local.
5. Type a small edit in the opened document, then close/discard it. Do not test save in Phase 19.

### Phase 19 Windows Markdown open-dialog smoke contract

Phase 19 starts from this baseline:

- Working today: command-first launch, `smoke-gui`, foreground server/client validation, local optimistic typing, server-owned workspace/file opens for configured roots, the `markdown-mode` fixture that loads `@clay/markdown`, activates `sample.md`/document `1`, publishes representative Markdown decorations, shows inert Markdown status SDUI, the bindable `clay.documents.clientOpenFileDialog` client UI command, the Windows native dialog backend that filters for `.md`, `.markdown`, and `.mdown` plus an all-files fallback, explicit selected-file IPC, server single-file grants for files outside configured workspace roots, buffer replacement from the selected-file open response, and live selected-file Markdown activation/decorations/status when `@clay/markdown` is loaded.
- Save exists for Phase 9 workspace documents, but saving a file picked through the Phase 19 dialog is not part of this manual smoke contract.

The in-scope manual Windows 11 smoke scenario is edit-only:

1. Load the first-party Markdown package and configure the key binding through `~/.config/clay/init.js`, or use the repository fixture with `cargo run -- smoke-gui --config-fixture windows-markdown-open`:

   ```js
   import { bindKey } from "clay:keybindings";

   bindKey("Ctrl+O", "clay.documents.clientOpenFileDialog", { scope: "editor" });
   ```

2. Launch Clay with the normal command-first GUI path or the fixture command above.
3. Press the configured `Ctrl+O` binding. On Windows 11, Clay should open the OS file browser with Markdown filters for `.md`, `.markdown`, and `.mdown` plus an all-files fallback.
4. Select a regular UTF-8 Markdown file. Cancellation should be a non-error no-op.
5. Clay should send the selected path to the server as an explicit user-selected open request. The server validates and grants only that file, opens it as a Clay document, replaces the current buffer snapshot, activates Markdown mode when `@clay/markdown` is loaded, and publishes viewport-bounded Markdown decorations/status.
6. Type in the opened document and confirm local editing remains responsive while decoration refresh may arrive asynchronously.
7. Do not test save for this phase; close or discard the smoke document after editing.

Out of scope for Phase 19 smoke: saving the selected file, full HTML preview or browser/webview rendering, non-Windows native dialogs, Windows Explorer file associations, double-click-to-open behavior, drag-and-drop, recent-files lists, directory opens, package installation, network fetches, shell execution, workspace expansion to the selected file's parent directory, and client-side package JavaScript.

Performance and security contract: the explicit open-dialog command may perform modal native UI and server file-open work. Ordinary typing, paint, scroll, layout, and text-event paths must remain client-local/non-blocking and must not wait on JavaScript, IPC, file IO, parser work, or full-document serialization. A selected path is an explicit user-mediated open request only; it is not unrestricted client filesystem authority and must not broaden workspace access beyond the selected regular UTF-8 file.

On non-Windows platforms during Phase 19, the command should report an unsupported diagnostic/status without panics; non-Windows native dialogs are intentionally not part of the smoke contract.

### Foreground server plus clients

Use the default server/client commands to validate second-client observer behavior without endpoint arguments:

```bash
cargo run -- server
cargo run -- client
cargo run -- client
```

The first client should show `Connected — Editable`; the second should show `Connected — Read-only Observer`.

## Runtime Diagnostic Smoke Expectations

To manually validate runtime diagnostics, temporarily use an invalid local configuration such as a syntax error in `~/.config/clay/init.js` or an unauthorized import. Start the foreground server and GUI client:

```bash
cargo run -- server
cargo run -- client
```

Expected behavior:

- The server logs a `clay.runtime.*` or `clay.configuration.*` diagnostic code with safe detail.
- The GUI status line includes `Runtime <code>: <message>` after connection.
- The status message omits raw absolute paths and source snippets.
- Typing and native rendering remain responsive; diagnostics are status events, not synchronous input/rendering work.

## Security and Endpoint Boundaries

Default and smoke launch paths use only local IPC transports:

- Windows: local named pipes.
- Unix: Unix domain sockets.

Normal GUI smoke validation does not open a remote TCP listener, does not use shell-mediated IPC, and does not require user-managed endpoints. Child servers are launched with `std::process::Command`-style direct executable arguments rather than through a shell. The `--config-fixture runtime-sdui`, `--config-fixture markdown-mode`, and `--config-fixture windows-markdown-open` development options resolve only named repository fixtures under `tests/fixtures/configuration/`; they do not enable package installs, network fetches, shell execution, arbitrary client JavaScript, WASM, AI mutation, or direct client filesystem authority. The Markdown fixtures register only the package commands they use before publishing SDUI actions; the Windows Markdown open fixture also binds the native dialog command through inert keybinding configuration.

Advanced endpoint arguments are optional debugging aids only, for example when reproducing a specific bind/listen problem or inspecting a custom endpoint. They are not part of normal GUI smoke validation.

## Implementation Details

For code-level behavior, see the [Client/Server Edit Acknowledgement Flow](../wiki/flows/client-server-edit-ack.md), [Client Snapshot Bootstrap](../wiki/modules/client-snapshot-bootstrap.md), and [Server IPC Skeleton](../wiki/modules/server-ipc-skeleton.md).
