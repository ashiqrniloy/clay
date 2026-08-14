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
| L10 | Put a syntax error in `~/.config/clay/init.js`, restart | GUI still opens; status/terminal shows `runtime.syntax_error` diagnostic, previous generation behavior retained as documented |
| L11 | No server reachable for a client-only invocation | `Local Fallback` state |

## Plan 087 UI foundation steps

| # | Action | Expected |
|---|--------|----------|
| L12 | Fresh isolated launch with an empty-document tab (no restore) | Welcome entry state shows instead of a stale prototype document: `Welcome to Clay` group with `Open File` / `Open Folder` buttons, polite status `Ready to edit; Open a file or folder to start editing.; Workspace: <basename>; Connection: Connected; Access: Editable.`; status bar shows `Connected — Editable`; no ambient config/socket used |
| L13 | Review harness launch: `scripts/capture-ui-review.sh --fixture ui-review-default --output <dir>` | Documented repeatable command (module reference `docs/development/launch-and-gui-smoke.md`) boots isolated server+client, captures AT-SPI dump + screenshot, writes `review.status PASS`; UNRESOLVED (exit 2) with a stated reason when the desktop accessibility bus is unavailable — never a false pass |
| L14 | Watch the AT-SPI tree while idle (welcome state) | No `Welcome to Clay's Phase 4 IPC server.` copy anywhere; entry/status labels contain no absolute paths |

## Negative checks

- Status line never shows absolute paths, source snippets, secrets, tokens,
  or env dumps (sanitize contract).
- Typing never blocks on IPC: keystrokes stay local-optimistic even while
  `Pending edits` > 0 or after disconnect.

## Linux execution record (Plan 086 task 11, 2026-08-14)

- **PASS — L6/L7/L8:** isolated `clay server <temp-socket>` + `clay client <temp-socket>` launch (HOME, XDG config/data, and socket under a mode-700 `/tmp/clay-plan086-manual-*` root) produced a live AT-SPI `clay` application. The initial tree showed `Connected — Editable`, version text, two restored tabs, two panes, the attached `Server-driven UI region`, and no startup panic. Text insertion updated the document/status tree and the server/client stayed alive.
- **PASS — isolation/negative:** the custom `init.js` was loaded from the isolated HOME (the client log contained the custom `Ctrl+O`, `Ctrl+S`, `Ctrl+Alt+P`, and `Alt+P` bindings); no ambient `~/.config/clay` or default socket was used. AT-SPI labels exposed basenames/status text, not host paths or secrets.
- **Coverage note:** observer/restart/local-fallback flows (L2–L5, L9–L11) were not re-run in this pass; their automated/Task 8 live coverage remains unchanged. Native window focus/input limitations blocked a clean second-client keyboard run.

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — L12/L14 welcome entry state:** fresh isolated launch (mode-700 root, X11-backend client, `ui-review-completion` fixture init.js, no restored document) showed the Clay-owned welcome: Frame `Clay` (active+focused), `Pane 1 of 1: editor`, `Welcome to Clay` panel with polite status `Ready to edit; Open a file or folder to start editing.; Workspace: workspace; Connection: Connected; Access: Editable.` plus `Open File`/`Open Folder` buttons and status bar `Clay — Connected — Editable — doc 2 — v1`. No `Phase 4 IPC server` copy and no absolute path in any AT-SPI label. Server/client stayed alive throughout.
- **PASS — L13 harness contract:** the documented `scripts/capture-ui-review.sh` flow was exercised earlier this plan (task 7/2 evidence: default/loading/error/recovery/completion/command-centre all `review.status PASS`, UNRESOLVED reported for interactive states that could not be driven).

## Known ceilings

- `cargo run -- restart` is Linux-only; other platforms return an
  unsupported-command error.
- Observer cannot gain the editable lease until the editable client
  disconnects cleanly; lease handover timing is out of scope here.
