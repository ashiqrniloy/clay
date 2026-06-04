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

# Foreground default server, useful for watching server diagnostics.
cargo run -- server

# First default client: should receive the editable lease when available.
cargo run -- client

# Second default client: should attach as a read-only observer.
cargo run -- client
```

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

Normal GUI smoke validation does not open a remote TCP listener, does not use shell-mediated IPC, and does not require user-managed endpoints. Child servers are launched with `std::process::Command`-style direct executable arguments rather than through a shell. The `--config-fixture runtime-sdui` and `--config-fixture markdown-mode` development options resolve only named repository fixtures under `tests/fixtures/configuration/`; they do not enable package installs, network fetches, shell execution, arbitrary client JavaScript, WASM, AI mutation, or direct client filesystem authority. The Markdown fixture registers only the package commands it uses before publishing SDUI actions.

Advanced endpoint arguments are optional debugging aids only, for example when reproducing a specific bind/listen problem or inspecting a custom endpoint. They are not part of normal GUI smoke validation.

## Implementation Details

For code-level behavior, see the [Client/Server Edit Acknowledgement Flow](../wiki/flows/client-server-edit-ack.md), [Client Snapshot Bootstrap](../wiki/modules/client-snapshot-bootstrap.md), and [Server IPC Skeleton](../wiki/modules/server-ipc-skeleton.md).
