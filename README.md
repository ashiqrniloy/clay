# Clay

Clay is a client/server code editor. See [Clay documentation](docs/index.md) and the [launch guide](docs/development/launch-and-gui-smoke.md).

## Run from a source checkout

```bash
cd frontend && npm ci && npm run build
cd .. && cargo build -p clay -p clay-desktop
cargo run
```

`cargo run` builds only the `clay` launcher. It reuses sibling
`clay-desktop`/`clay-server` executables, and the desktop embeds
`frontend/dist`; run the build steps above after source changes so the GUI and
server are not stale. Bare `cargo run` then replaces the default-endpoint
server and opens the desktop.

## Fully restart the server on Linux

Use Clay's restart command. It stops only the default server, starts a fresh background server, waits for readiness, then exits without opening another GUI:

```bash
clay restart

# From this repository:
cargo run -- restart
```

Manual fallback—stop only Clay server processes (not GUI clients):

```bash
pkill -TERM -f '^(.*/)?clay server([[:space:]]|$)'
```

Confirm the server stopped:

```bash
pgrep -af '^(.*/)?clay server([[:space:]]|$)' || echo "Clay server stopped"
```

If it ignores `SIGTERM`, force-stop it:

```bash
pkill -KILL -f '^(.*/)?clay server([[:space:]]|$)'
```

A new server safely replaces a stale Unix socket. If no server remains but startup still reports an occupied endpoint, remove the stale default socket manually:

```bash
rm -f "${XDG_RUNTIME_DIR:-/tmp/clay-$USER}/clay.sock"
```

Then start a fresh server and GUI from already-built artifacts:

```bash
cd /path/to/clay
cargo run
```

For foreground server diagnostics, use two terminals:

```bash
# Terminal 1
cargo run -- server

# Terminal 2
cargo run -- client
```
