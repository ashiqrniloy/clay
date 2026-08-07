# 01 — Launch and Connection

Verify the server/client lifecycle, connection states, and the status line.
Deep reference: `docs/development/launch-and-gui-smoke.md`.

## Setup

```bash
cargo build
# No init.js required for this module; a broken/missing init.js is part of the test.
```

## Startup

| # | Action | Expected |
|---|--------|----------|
| L1 | `cargo run` with no server running | Clay spawns a background `clay server <endpoint>` itself, GUI opens after handshake |
| L2 | `cargo run` again while the first is running | Second GUI attaches as `Connected — Read-only Observer` (editable lease belongs to first client) |
| L3 | Close the first (editable) client | Observer remains attached; status does not crash/lie about lease |
| L4 | `cargo run -- restart` | Kills only the default-endpoint server of this executable, starts fresh, waits readiness, exits without GUI |
| L5 | `cargo run -- smoke-gui` | Isolated endpoint + managed child server; child terminates when GUI exits |
| L6 | `cargo run -- smoke-gui --config-fixture runtime-sdui` | Server-generated panel + editor view render; connection status visible |
| L7 | Quit a multi-tab session normally (window close), relaunch `cargo run` | The window restores from `layout.json` v2 after the handshake — same tabs/workspaces/layouts/documents (full sequence in module 14 T41; a corrupt or missing file launches the default single-tab window) |

## Status line and diagnostics

| # | Action | Expected |
|---|--------|----------|
| L7 | After L1 | Status shows `Connected — Editable`; version text like `vN` |
| L8 | Type text | `Pending edits: N` increments then decrements after ack |
| L9 | Kill the server process while GUI open | `Disconnected` + recovery/dismiss menu; no raw paths/host strings leaked |
| L10 | Put a syntax error in `~/.config/clay/init.js`, restart | GUI still opens; status/terminal shows `clay.runtime.syntax_error` diagnostic, previous generation behavior retained as documented |
| L11 | No server reachable for a client-only invocation | `Local Fallback` state |

## Negative checks

- Status line never shows absolute paths, source snippets, secrets, tokens,
  or env dumps (sanitize contract).
- Typing never blocks on IPC: keystrokes stay local-optimistic even while
  `Pending edits` > 0 or after disconnect.

## Known ceilings

- `cargo run -- restart` is Linux-only; other platforms return an
  unsupported-command error.
- Observer cannot gain the editable lease until the editable client
  disconnects cleanly; lease handover timing is out of scope here.
