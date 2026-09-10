# Plan 117 visual screenshot + accessibility review (2026-09-10)

Scratch-config launch (isolated roots, never the ambient profile; no secrets
in any capture). Captures are window-cropped portal screenshots; host windows
and paths excluded.

## Captures (inspected)

- `window-connected.png` / `window-connected-fresh.png` — healthy connected
  shell at 1280×1151 (fresh relaunch, zero wire errors): CLAY | Workspace tab
  bar, file tree, CodeMirror document with syntax highlighting, status bar
  `Workspace · Connected`. No clipping, overlap, or unreadable contrast; tab
  strip and file tree render the selected states correctly.
- `window-narrow-760.png` — 760×1151 narrow layout: tab strip truncates
  gracefully, file browser narrows, editor text wraps without overflow, status
  bar intact (plan 109 narrow-layout behavior preserved).
- `atspi-frame-dump.txt` — AT-SPI tree of the running app.

## Accessibility assessment

- Automated legs PASS: the panel suite pins accessible structure at the jsdom
  level — `getByRole("tab")` ×10, `button` ×9, `region` ×2, `log` ×1 (plus
  aria-labels on icon-only actions) across CodingAgentPanel tests (294-test
  suite green).
- Live AT-SPI finding (defect candidate): the WebKitGTK web content subtree is
  NOT bridged to AT-SPI in this build/session — the dump exposes only the
  frame + native window chrome (14 nodes), reproducing across two fresh
  launches. Plan 109 (2026-09-06) recorded a full webview subtree, so this is
  a regression candidate, most likely environmental (WebKitGTK a11y
  enablement for this launch context). Follow-up recorded in the dump.
- Because no web node exposes AT-SPI actions and no input-synthesis backend is
  available on this host (no uinput perms, no ydotoold socket, RemoteDesktop
  portal without pointer capability), the changed-surface states (skills/MCP
  cards, settings page, @ dropdown, token meter bands, branch/effort/resume)
  could not be driven live. Their rendering + roles are pinned by the frontend
  suite; the server-side halves were verified through the real daemon (module
  17 plan 117 record). A human click-through after restoring the webview a11y
  bridge completes the live matrix (plan 109 attempt-3 precedent).
