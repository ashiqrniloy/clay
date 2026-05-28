# Launch and GUI Smoke Validation

Use these command-first launch paths to validate Clay's current GUI and client/server behavior on every supported desktop platform. The normal workflow does not require copying a named pipe or Unix socket path.

## Quick Commands

From the repository root:

```bash
# Start or reuse the default local server, then open the GUI client.
cargo run

# App-managed GUI smoke run with an isolated endpoint and managed child server.
cargo run -- smoke-gui

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

Typing remains local and optimistic. Editor input must not wait for IPC acknowledgements, server work, or full-document synchronization; acknowledgements and resyncs arrive asynchronously and update status when available.

## Smoke Modes

### Bare `cargo run`

Bare `cargo run` tries the platform default local endpoint. If no server is reachable, Clay starts the current executable directly as a background `clay server <endpoint>` process, retries the client handshake for a bounded readiness window, and opens the GUI when connected.

### `cargo run -- smoke-gui`

`smoke-gui` is the isolated app-managed GUI smoke path. It creates a unique local endpoint, starts a managed child `clay server <endpoint>` process with direct arguments, waits for readiness, opens the GUI client, and terminates the child server when the GUI exits.

### Foreground server plus clients

Use the default server/client commands to validate second-client observer behavior without endpoint arguments:

```bash
cargo run -- server
cargo run -- client
cargo run -- client
```

The first client should show `Connected — Editable`; the second should show `Connected — Read-only Observer`.

## Security and Endpoint Boundaries

Default and smoke launch paths use only local IPC transports:

- Windows: local named pipes.
- Unix: Unix domain sockets.

Normal GUI smoke validation does not open a remote TCP listener, does not use shell-mediated IPC, and does not require user-managed endpoints. Child servers are launched with `std::process::Command`-style direct executable arguments rather than through a shell.

Advanced endpoint arguments are optional debugging aids only, for example when reproducing a specific bind/listen problem or inspecting a custom endpoint. They are not part of normal GUI smoke validation.

## Implementation Details

For code-level behavior, see the [Client/Server Edit Acknowledgement Flow](../wiki/flows/client-server-edit-ack.md), [Client Snapshot Bootstrap](../wiki/modules/client-snapshot-bootstrap.md), and [Server IPC Skeleton](../wiki/modules/server-ipc-skeleton.md).
