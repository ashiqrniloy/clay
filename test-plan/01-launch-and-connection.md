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

## Plan 088 modernization steps

| # | Action | Expected |
|---|--------|----------|
| L15 | Run `scripts/capture-ui-review.sh --fixture ui-review-default --output <dir>` on the current Linux build and inspect the Clay-only screenshot/tree | Welcome shell is bounded at the documented 900×600 logical window; Open File/Open Folder have names; status exposes Connected/Editable; no host path appears |
| L16 | Compare dark and light theme welcome captures, then reload with the large-typography fixture (`ui` size 24) | Surface/text hierarchy remains legible and in bounds in both themes; UI typography changes geometry without changing user-owned font-family policy |
| L17 | Inspect the runtime-error fixture | Error copy appears in accessible panel/status names and is not conveyed by color alone; client remains usable |
| L18 | Inspect the disconnect/recovery fixture | Welcome, recovery panel, and status chrome all report Disconnected consistently; no absolute path or secret appears |
| L19 | Inspect the loading fixture | The published loading state is observable in the tree and screenshot, or the harness records `UNRESOLVED` with the fixture/observability reason rather than claiming a pass |

## Plan 088 task 12 Linux execution record (2026-08-15)

Real Linux/GNOME Wayland execution used `cargo build`, the isolated mode-700 review harness, xdg-desktop-portal PNG capture, and Python GI/AT-SPI dumps. Window targeting remains unavailable, so no targeted keyboard or native-dialog action was claimed as a pass.

| Checks | Result | Evidence |
|---|---|---|
| L15 | PASS | Current-build artifact: `code-reviews/screenshots/2026-08-15-plan088-task12-manual/default/` (`review.status PASS`, 900×600 logical metadata, Clay-only 913×1152 crop); tree exposes Welcome to Clay, Open File/Open Folder, Connected/Editable status, and no absolute path |
| L16 | PASS | Existing same-build dark/light and large-typography artifacts: `code-reviews/screenshots/2026-08-14-plan088-modernization/light-default/` and `large-typography/`; no a11y regression, with visual large-type sizing recorded |
| L17 | PASS | `code-reviews/screenshots/2026-08-14-plan088-modernization/error/`; diagnostic is present in panel and status accessible names, not color-only |
| L18 | FAIL — P1 follow-up | `code-reviews/screenshots/2026-08-14-plan088-modernization/recovery/`; recovery/status chrome says Disconnected but the WelcomeWidget status still says Connected. Track as a product defect, not a false pass |
| L19 | UNRESOLVED — observability gap | `code-reviews/screenshots/2026-08-14-plan088-modernization/loading/` captured the welcome shell instead of the intended loading SDUI tree; harness pass alone is insufficient |

## Plan 089 validation and platform steps

| # | Action | Expected |
|---|--------|----------|
| L20 | Launch two real Clay desktop clients on a Wayland host with large-typography init.js (ui 24/mono 20/proportional 21) and dump AT-SPI per instance | Two distinct `clay-desktop` frames are exposed with positive physical bounds and scale factors between 0.5 and 4.0. (The automated `live_atspi_smoke::live_multi_window_scale_smoke` harness was removed with the native client; this step is now manual-only.) |
| L21 | Resize the desktop window across scale changes (e.g. 1×→2× display scale) and confirm layout stays in bounds; cross-check the Plan 097 Phase 12 wide/narrow captures | Tab bar, panes, status bar, and dialogs stay inside window bounds after rescale; no clipped chrome. (The headless Masonry rescale unit test was removed with the native client; responsive coverage now comes from this manual check plus the retained fixture captures.) |
| L22 | Inspect the `ui-review-large-typography` fixture capture (`scripts/capture-ui-review.sh --fixture ui-review-large-typography --output <dir>`) | Welcome state with large UI typography (size 24/20/21) renders in bounds; Open File/Open Folder buttons, status bar, and polite status remain legible and accessible; no absolute path leaks |

## Plan 089 task 9 Linux execution record (2026-08-17)

Real Linux/GNOME Wayland execution used `cargo build`, the isolated mode-700 review harness, xdg-desktop-portal PNG capture, Python GI/AT-SPI dumps, and the now-active GNOME Shell extension for window targeting (`can_query_windows=true`, `can_focus_windows=true`).

| Checks | Result | Evidence |
|---|---|---|
| L18 | PASS | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/recovery/` shows `Connection lost` / `Connection: Disconnected` consistently in the welcome panel, status chrome, and AT-SPI tree after the `request_welcome_render` fix |
| L19 | PASS (delivered-RuntimeStateSnapshot evidence) | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/loading/` with `runtime-tree.txt` confirming the published loading SDUI tree (Panel `Loading review`, Label `Loading workspace…`) was delivered via `RuntimeStateSnapshot`; the restore-gate fix and kind-changed reconcile fix ensure the tree reaches the accessibility layer |
| L20 | PASS live | `CLAY_LIVE_WINDOW_SMOKE=1` multi-window smoke test launched two real Clay clients; AT-SPI exposed two PID-separated frames with positive bounds and scale factors within 0.5–4.0 |
| L21 | PASS headless | `rescale_event_recomputes_logical_bounds_from_physical_size` passes; logical size remains 900x600 at 2x physical scale |
| L22 | PASS | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/large-typography/` shows the large UI fixture in bounds with named welcome actions and Connected status |

## Known ceilings

- `cargo run -- restart` is Linux-only; other platforms return an
  unsupported-command error.
- Observer cannot gain the editable lease until the editable client
  disconnects cleanly; lease handover timing is out of scope here.

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Launch/welcome shell | PASS real AT-SPI structure | `code-reviews/screenshots/2026-08-24-tauri-react-parity/default-welcome/accessibility.txt` exposes the Tauri frame, `Clay workspace`, `Window tabs`, `Workspace`, `Pane 1`, and named Open File/Open Folder actions |
| Opened editor shell | PASS real AT-SPI structure | `editor-opened/accessibility.txt` exposes Save/Reload/Close, Open path, Open, `Editor notes.md`, and the Document editor entry |
| Loading/empty/error/recovery | PASS static visual/a11y coverage | `states/fixture-{wide,narrow}.png` and paired AX snapshots |
| Physical keyboard/reconnect re-run | UNRESOLVED host | `/dev/uinput` denied; no xdotool/ydotool; Wayland portal cannot target Clay. No interactive pass inferred |

The retained screenshots are app-only CDP captures; full-desktop portal PNGs
with unrelated windows were deleted. See the dated review log for all state
results and cleanup policy.

## Plan 098 chunked document transfer steps

| # | Action | Expected |
|---|--------|----------|
| L23 | Start `scripts/large-document-smoke.sh` and inspect its private `clay server <socket>` plus Tauri client launch | Current Linux build completes a protocol-v27 handshake on the workspace-private socket; status reaches Connected; no default-endpoint server is adopted |
| L24 | If a v26 server binary is available, connect the current client to it; otherwise run `cargo test --lib protocol_v26_client_is_rejected_by_v27_server` | Mixed protocol versions fail closed with `UnsupportedProtocolVersion`; no document/workspace state is installed and the current client remains recoverable |

## Plan 098 Linux execution record (2026-08-26)

| Checks | Result | Evidence |
|---|---|---|
| L23 | PASS launch / UNRESOLVED interactive close | `scripts/large-document-smoke.sh` built `target/debug/clay`, started a server on a private temporary socket, and launched the Tauri desktop. The welcome screenshot is `code-reviews/screenshots/2026-08-26-plan098-manual/real-app-welcome.png`; no default socket was used. Portal/window targeting became unstable before a stable document state, so no live editor interaction pass is claimed |
| L24 | PASS automated; NOT RUN against a separate live v26 binary | `cargo test --lib protocol_v26_client_is_rejected_by_v27_server` passed; no v26 server executable was available for a second live process |

Known ceiling for this record: AT-SPI exposed the native Tauri frame but not
WebKitGTK document nodes, and the host's compositor moved the test window
partly off-screen during portal focus. These conditions leave live L23
editor interaction unresolved rather than converting protocol evidence into a
GUI pass.
