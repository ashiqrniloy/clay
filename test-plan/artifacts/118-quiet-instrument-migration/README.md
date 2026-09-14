# Plan 118 manual-test-plan evidence (2026-09-13)

One directory per review state, captured with
`scripts/capture-ui-review.sh --fixture <name> --output <dir>` on the freshly
rebuilt Linux desktop build (`cargo build --bins` **and**
`cargo build --bins -p clay-desktop` — `clay-desktop` lives in `src-tauri`, so
the first command alone leaves it stale and the client dies with `Session lost`).

Each state directory contains: `review.status` (`PASS`/`UNRESOLVED` + fixture),
`accessibility.txt` (AT-SPI tree dump), `screenshot.png` (window-cropped —
full-desktop portal captures are never retained), `metadata.txt`,
`instructions.md`, and `runtime-tree.txt` where the fixture publishes one. The
harness runs with a mode-700 HOME/XDG root, a private socket, and its own
workspace, so nothing here contains host paths or user content beyond the
launcher's own recents row (which is the isolated workspace path).

| Directory                                     | Fixture                           | Covers                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------------------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `launcher-landing/`                           | `ui-review-launcher` (new)        | empty-tab landing from `@clay/launcher`: `Start`, panes, recents row, first-run notes, action footer                                                                                                                                                                                                                                                                                                                                                   |
| `core-fallback/`                              | `ui-review-default`               | empty tab with **no** contribution: `Start with a file or folder`, `Open file`/`Open folder`, no product name                                                                                                                                                                                                                                                                                                                                          |
| `design-system-dark/`, `design-system-light/` | `ui-review-design-system(-light)` | shipped `@clay/design-instrument` activation under dark/light, host-owned enabled/disabled states                                                                                                                                                                                                                                                                                                                                                      |
| `coding-agent/`                               | `ui-review-coding-agent`          | launch state only — the agent **view** stays UNRESOLVED (package-contributed command + no input synthesis); view evidence is the DEV-harness set recorded under UI-DS-35                                                                                                                                                                                                                                                                               |
| `loading/`, `error/`, `recovery/`             | fixtures of the same name         | delivered loading snapshot, sanitized reload diagnostic with the client still connected, disconnected/recovery state                                                                                                                                                                                                                                                                                                                                   |
| `large-typography/`                           | `ui-review-large-typography`      | ui 24 / mono 20 / proportional 21 in bounds                                                                                                                                                                                                                                                                                                                                                                                                            |
| `review/`                                     | the matrix above                  | the visual and accessibility review of the migrated app: 42 captured states (4 themes × 2 widths for the primary scenes, both widths for the state scenes), each with a screenshot, its accessibility tree, its drive steps and measurements, plus `visual-review.md` (verdicts, deviation dispositions, defect log, unresolved items), `pixel-contrast.txt` (live hairline contrast) and the live AT-SPI walk (`a11y-live.txt`, `a11y-live-tree.txt`) |

Not reproducible from these files alone: the legs listed as UNRESOLVED in the
module records (landing handoffs, close-return, live theme switch, the
unsaved-changes sheet, live tooltips, the agent pane in the desktop build).
AT-SPI action invocation works on this host and the harness now drives it
(`--drive '<json steps>'`), which is how the palette, native-dialog, rail-toggle
and focus captures in `review/` were produced; the remaining legs need input the
session cannot synthesize (pointer entry for tooltips, a document edit for the
sheet) or a bridge-mocked pane fixture, and the plan carries them as follow-ups.
The agent-view visual evidence remains the dev-route matrix in
`design-artifacts/screenshots/quiet-instrument-agent/`.
