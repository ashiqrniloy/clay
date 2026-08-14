# Repeatable UI Review Harness (Plan 087)

## Source

- `scripts/capture-ui-review.sh`
- `tests/fixtures/configuration/ui-review-*` (six deterministic fixtures)
- `tests/manual_smoke_docs.rs` — command/fixture documentation drift guard
- `docs/development/launch-and-gui-smoke.md` — harness documentation
- `docs/development/ui-observability.md` — observability entry point
- `plans/087-Audit-Remediation-UI-Foundation-and-Review-Harness.md`

## Overview

Plan 087 adds one documented command that launches isolated, fixed-size Linux GUI fixtures for repeatable state capture. The harness exists because `smoke-gui` mode forces a smoke endpoint and applies no window restore or HOME/XDG isolation, so it cannot represent normal end-user entry states (welcome, restored documents, completion, Command Centre). The harness is a review workflow, not a CI golden-image system: screenshots are review artifacts, and GPU pixel snapshots stay deferred (Masonry's `TestHarness` is CPU-only and not production-renderer faithful).

## How It Works

### Command

```bash
scripts/capture-ui-review.sh --fixture ui-review-default --output <artifact-dir>
```

Optional `--timeout <seconds>` (default 45, `CLAY_UI_REVIEW_TIMEOUT_SECONDS`). The script:

1. Creates a mode-700 `mktemp` root with isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `TMPDIR`.
2. Copies the fixture `init.js` to `$home/.config/clay/init.js` and, for document-bearing fixtures, writes `layout.json` v2 with an explicit leaf-form `splitTree` (`{"leaf":{"paneId":1}}` — a null `splitTree` degrades to the default single-pane layout and never reopens documents).
3. Spawns `clay server <socket>` (no `--config-fixture`; that flag is bypassed because it skips the watcher-reload path that fixtures depend on) then `clay client <socket>`. `--config-fixture` is deliberately not used: fixture `init.js` is only evaluated through watcher-driven reloads, so the script copies the file into the isolated config root and touches it after the socket appears to force one reload.
4. Polls an embedded python3 GI-Atspi probe for the named state, then records `metadata.txt`, `instructions.md`, `accessibility.txt`, `screenshot.png`, and `review.status` into `--output`.

Exit codes: `0` with `review.status PASS` on success; `2` with an explicit reason (`UNRESOLVED`) when the fixture state cannot be reached or the desktop accessibility bus is missing — never a false pass. Interactive TTY states (completion, Command Centre) are recorded `UNRESOLVED` off a TTY with their reasons.

### Fixtures

| Fixture | init.js content | State captured |
|---|---|---|
| `ui-review-default` | empty comment | welcome entry state (empty-tab bootstrap) |
| `ui-review-loading` | static SDUI `Loading workspace…` panel | published loading panel via watcher reload |
| `ui-review-error` | `setTheme('@clay/does-not-exist')` | sanitized `Runtime packages.not_installed` diagnostic, usable shell |
| `ui-review-recovery` | empty comment | disconnected/reconnect-guidance state |
| `ui-review-completion` | `loadPackage('@clay/rust')` + `completion.trigger` on `Ctrl+Space` | completion popup (interactive) |
| `ui-review-command-centre` | `controlCenter.open` on `Ctrl+Alt+P` | centered Command Centre (interactive) |

The probe first locates the `clay` application index by scanning desktop children (`app INDEX` with per-call timeouts — whole-desktop enumeration hangs on some hosts), then dumps only that subtree. Hosts without `python3` + `gi.repository.Atspi` are reported as a prerequisite skip, never a pass.

### Window backend note

Keyboard-driven captures on the review host require the X11 backend: launching the client with `WINIT_UNIX_BACKEND=x11` gives the clay Frame AT-SPI `active`+`focused` states and lets xdg-desktop-portal key delivery reach the editor. Wayland sessions deliver portal keys only to native dialogs, not clay windows. Multi-stroke chords (`Ctrl+X Ctrl+P`) cannot be delivered through the portal (each combo arrives as press+release and the pending-chord timeout is ~1.5 s); the Command Centre capture instead re-binds `controlCenter.open` to a single chord, or reuses a prior live capture of the same build.

## Review Artifacts

Captured runs live under `code-reviews/screenshots/<run>/` with `review.status` per state plus a `review-log.md` state table (screenshot/AT-SPI result/verdict per state, numbered findings, and an explicit "no screenshot contains secrets or absolute paths" statement). Example evidence trees:

- `code-reviews/screenshots/2026-08-14-plan086-a11y/` — plan 086 accessibility review
- `code-reviews/screenshots/2026-08-14-plan087-ui-foundation/` — plan 087 default/loading/error/recovery/opened-document/completion/empty-completion/command-centre captures

Screenshots are full-desktop portal PNGs cropped to the clay window with a pure-stdlib cropper (the host has no imaging libraries). PNGs are review evidence, not CI goldens.

## Invariants and Constraints

- Every run uses a fresh mode-700 root: no ambient `~/.config/clay`, no default socket, no ambient server/config.
- Server and client are killed and the root removed on every exit path (timeout included).
- A missing accessibility bus or unreachable state yields `UNRESOLVED` with a reason (exit 2), never `PASS`.
- No screenshot/AT-SPI capture may contain document secrets or host paths; fixture workspaces hold only review files.
- The drift guard `plan087_ui_review_harness_command_and_prerequisites_are_documented` in `tests/manual_smoke_docs.rs` (protocol suite) re-asserts the documented commands, the six fixture `init.js` files, and script safety markers, and forbids `cargo run -- smoke-gui` as a review substitute.

## Related

- [docs/development/launch-and-gui-smoke.md](../../development/launch-and-gui-smoke.md) — harness reference (fixture/state/capture table, `WINDOW_WIDTH`/`WINDOW_HEIGHT` constants, UNRESOLVED semantics)
- [docs/development/ui-observability.md](../../development/ui-observability.md) — observability entry point
- [Masonry Shell Runtime](masonry-shell.md) — shell/chrome hosting the states the harness captures
- [Pane Document Views](pane-document-views.md) — welcome entry state and completion projection
- [Centered Command Centre Surface](centered-command-centre-surface.md) — the centered modal the harness captures
- [test-plan/index.md](../../test-plan/index.md) — manual step IDs per state (L12–L14, F32–F37, E16–E21, K69–K72, Q11–Q14, S33–S35)
