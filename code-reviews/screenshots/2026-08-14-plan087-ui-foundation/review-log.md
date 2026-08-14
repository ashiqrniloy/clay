# Plan 087 task 7 — Linux visual/accessibility review

Date: 2026-08-15
Host: Linux/GNOME Wayland desktop, live AT-SPI bus, X11 Clay review client
using `WINIT_UNIX_BACKEND=x11` so the existing active-window path could
receive keyboard input. `get_app_state` was called before interaction.

## Isolation and method

- Clay server/client used a private Unix socket, mode-700 temporary HOME/XDG
  config/data/TMP roots, and fixture-only synthetic `review.md` content.
- Default/loading/error/recovery states came from the repeatable harness; the
  opened-document/completion/Command Centre states were then exercised against
  the same isolated fixture with the native Linux file dialog and keyboard
  chords.
- Portal screenshots were captured full-screen, then cropped to the Clay
  window. Accessibility dumps contain Clay nodes only and retain no absolute
  workspace path.

## State evidence

| State | Evidence | AT-SPI result | Result |
|---|---|---|---|
| Default welcome | `default/screenshot.png`, `default/accessibility.txt` | Welcome group, Open File/Open Folder buttons, actionable connected status | PASS |
| Loading fixture | `loading/screenshot.png`, `loading/accessibility.txt` | Initial host tree is the welcome shell; the fixture's loading SDUI tree is confirmed in the watcher-reload log but is not exposed by this host's initial AT-SPI tree | PASS with observability limitation |
| Runtime error | `error/screenshot.png`, `error/accessibility.txt` | Usable shell with sanitized runtime diagnostic | PASS |
| Disconnected/recovery | `recovery/screenshot.png`, `recovery/accessibility.txt` | Disconnected headline and recovery guidance remain attached | PASS |
| Opened document | `opened-document/screenshot.png`, `opened-document/accessibility.txt` | Focused editable `review.md` Entry; status is `Connected — Editable`; labels expose basename only | PASS |
| Non-empty completion | `completion/screenshot.png`, `completion/accessibility.txt` | `Menu` `Completion`, 16 items, selected `Completion # selected`, `Recovery: Completion`, 480×340 logical popup | PASS with shared scroll-containment finding |
| Empty completion dismissal | `empty-completion/screenshot.png`, `empty-completion/accessibility.txt` | Live log received `CompletionResult { status: Empty, items: [] }`; no completion overlay or blocking empty panel remained | PASS |
| Command Centre, 66 results | `command-centre/screenshot.png`, `command-centre/accessibility.txt` | Centered modal `Dialog`/`Menu`, 66 semantic `MenuItem`s, selected item, `66 results`; package labels are sanitized (`@claymarkdown@0.1.0`) | PASS with shared scroll-containment finding |
| Command Centre filtered | `command-centre/filtered.png`, `command-centre/accessibility-filtered.txt` | Query `split` reduces catalogue to 8 results; selected item and `8 results` status remain visible | PASS with shared scroll-containment finding |
| Narrow/wide | no artifact | Safe window resize/targeting is unavailable on this host; no false visual pass claimed | UNRESOLVED prerequisite |

Keyboard interactions exercised: native Open File, `Ctrl+Space` completion,
completion empty-result mutation, default `Ctrl+X Ctrl+P` Command Centre open,
Command Centre query typing (`split`), and Escape dismissal. X11 delivery reached
Clay; the client and server stayed alive throughout.

## Findings and unresolved blockers

1. **Shared scroll containment is not visually contained in the live renderer.**
   `completion/screenshot.png` shows completion rows continuing below the
   480×340 popup shell, and `command-centre/screenshot.png` shows the 66-result
   list continuing below the 640×220 centered shell. The scrollbar is present,
   but child paint escapes the visible surface; the live accessibility update
   also reports semantic children whose logical bounds extend past the menu
   bounds. The structural scroll-offset/region-size tests pass, so they do not
   catch this renderer-level containment defect. **P1 follow-up required before
   Plan 087 can claim bounded visual completion/Command Centre containment.**

2. **Loading state is not observable through this host's initial AT-SPI tree.**
   The fixture publishes the loading SDUI tree during runtime reload, but the
   initial accessible tree captured by the host remains the welcome shell.
   Preserve the artifact caveat; do not treat it as a loading screenshot pass.

3. **Narrow/wide review remains blocked.** The host has no usable window-list or
   safe resize backend (`can_query_windows=false`, `can_focus_windows=false`).
   Blind portal/coordinate input is not a substitute; rerun when a supported
   Wayland window-target backend is available.

No screenshot contains secrets or absolute paths. No production change was kept
from this task; findings are recorded for the next implementation pass.
