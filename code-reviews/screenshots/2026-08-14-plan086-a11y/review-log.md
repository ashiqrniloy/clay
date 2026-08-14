# Plan 086 task 8 — Linux visual/accessibility review

Date: 2026-08-14
Host: Linux/GNOME Wayland, live AT-SPI bus, `computer-use-linux` portal input/screenshot.

## Isolation and method

- Clay server/client launched on an explicit isolated Unix socket with temporary mode-700 config/data homes.
- Only checked-in fixture names and synthetic text were used: `runtime-sdui`, `syntax-grammars`, and `hello hello_world helper`.
- `get_app_state` was called before interaction. Each keyboard mutation was followed by an AT-SPI re-query and, for each state below, a screenshot.
- Full-screen portal captures were cropped to the Clay window before retention, so unrelated desktop applications and host data are not in evidence images.

## State evidence

| State | Screenshot | AT-SPI result | Result |
|---|---|---|---|
| Default, one tab | `default-single-tab.png` | Shell panel `Clay working area shell. Active pane 1.`; `Pane 1 of 1: editor`; focused editable Entry; connected status | PASS |
| Restored multi-tab + status | `multi-tab-status.png` | `PageTabList` `Workspace tabs`; `runtime-sdui` and `syntax-grammars` `PageTab` children; exactly one selected; connected status | PASS |
| Multi-pane | `multi-pane.png` | `Pane 1 of 2: editor` plus `Empty pane 2 of 2`; status `Split pane vertically`; focused editor Entry remains attached | PASS |
| Control Center, unfiltered | `control-center-menu.png` | `Dialog`/`Menu` `Control Center`, selected menu item, result status; no tree rejection | PASS with visual finding below |
| Control Center, filtered | `control-center-filtered.png` | Query `split` reduced state to 7 visible list items; selected `Split Pane Down`; status `7 results`; Escape dismissed it | PASS |
| Completion empty-result state | `completion-empty.png` | Visible `Menu` named `Completion`; empty state says `No completions`; status carries `Recovery: Completion`; no crash | PASS with coverage limitation below |
| Announcement + selected tab | `announcement-status.png` | `syntax-grammars` selected; live `StatusBar` announcement `Switched to tab 2: syntax-grammars`; connected status remains | PASS; announcement is AT-SPI-only, not painted text |

Keyboard-only interactions exercised: tab close, Control Center open/filter/Enter/Escape, split-pane activation through the filtered command, text entry, completion trigger, and `Ctrl+2` tab activation. Normal representative flows stayed alive and produced attached AT-SPI trees.

## Findings and unresolved blockers

1. **Control Center overflow at the unfiltered 60-item state.** `control-center-menu.png` shows the menu/list extending below the 900x1116 Clay window and clipping at the bottom. Filtering to 7 results fits (`control-center-filtered.png`). This is a pre-existing visual/layout issue, not introduced by plan 086's stable virtual-node change. Follow-up belongs to Plan 087 completion/overlay geometry work.
2. **Completion provider returned empty in this fixture.** The Completion overlay and accessible empty state rendered correctly, but no `core.bufferWords` items appeared after inserting synthetic words. This review therefore proves the empty/recovery path, not successful item rendering; item geometry remains a Plan 087 follow-up. No malformed tree or process exit occurred.
3. **AT-SPI top-level-frame focus edge.** A tool-only `Atspi.Accessible.grab_focus()` on Clay's top-level Frame caused the client to exit with `Cannot send event to non-existent widget #8` (sanitized excerpt in `focus-frame-crash.log`). This is not part of ordinary keyboard-only use. Repeating focus through the real editor Entry (`grab_focus` on the Entry) was handled successfully and the normal keyboard flow stayed alive. Keep this as an accessibility-adapter follow-up before calling arbitrary top-level AT-SPI focus fully supported.
4. AT-SPI emitted non-fatal cache queries for unsupported `GetApplicationBusAddress`/`/org/a11y/atspi/cache`; the Clay tree remained queryable. This matches the earlier live-smoke observation and did not affect the pass/fail states above.

No screenshot contains document secrets or host paths. Temporary launch fixture and process homes were removed after capture; no fixture was committed.
