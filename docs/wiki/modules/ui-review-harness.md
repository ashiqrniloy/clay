# Repeatable UI Review Harness (Plan 087)

## Source

- `scripts/capture-ui-review.sh`
- `tests/fixtures/configuration/ui-review-*` (eight deterministic fixtures)
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
3. Spawns `clay server <socket>` (no `--config-fixture`; that flag is bypassed because fixtures depend on the watcher path) from the private fixture workspace, then `clay client <socket>`. The workspace cwd keeps bootstrap document IDs aligned with the loading SDUI binding. Fixture `init.js` is copied before launch, and the script touches it only after the client shell/handshake is observable so the runtime snapshot is delivered through the live connection.
4. Polls an embedded python3 GI-Atspi probe for the named state, then records `metadata.txt`, `instructions.md`, `accessibility.txt`, `screenshot.png`, and `review.status` into `--output`. The loading fixture additionally waits for exact `Loading review` / `Loading workspace…` fields in the delivered `RuntimeStateSnapshot` and writes `runtime-tree.txt`; it does not pass on a welcome-only tree.

Exit codes: `0` with `review.status PASS` on success; `2` with an explicit reason (`UNRESOLVED`) when the fixture state cannot be reached or the desktop accessibility bus is missing — never a false pass. Interactive TTY states (completion, Command Centre) are recorded `UNRESOLVED` off a TTY with their reasons. The default welcome capture is structural: it proves names, roles, bounds, and status text, but does not exercise mouse hit-testing or keyboard shortcuts; those paths are covered by the RenderRoot regressions `welcome_button_pointer_press_emits_open_file_command` and `welcome_global_keybindings_emit_commands_without_editing_text` in `src/masonry_editor.rs`.

### Fixtures

| Fixture | init.js content | State captured |
|---|---|---|
| `ui-review-default` | empty comment | welcome entry state (empty-tab bootstrap) |
| `ui-review-loading` | static SDUI `Loading workspace…` panel | published loading panel via watcher reload |
| `ui-review-error` | `setTheme('@clay/does-not-exist')` | sanitized `Runtime packages.not_installed` diagnostic, usable shell |
| `ui-review-recovery` | empty comment | disconnected/reconnect-guidance state |
| `ui-review-large-typography` | `setTypography` with UI 24 and document 20/21 | bounded large-type shell |
| `ui-review-completion` | `loadPackage('@clay/rust')` + `completion.trigger` on `Ctrl+Space` | completion popup (interactive) |
| `ui-review-command-centre` | `controlCenter.open` on `Ctrl+Alt+P` (single-stroke fixture override; not the shipped `Ctrl+X Ctrl+P` default) | centered Command Centre (interactive) |
| `ui-review-rust` | language-server authorization + `editor.toggleInlayHints` binding | Rust analyzer/inlay states (interactive) |

The probe first locates the `clay` application index by scanning desktop children (`app INDEX` with per-call timeouts — whole-desktop enumeration hangs on some hosts), then dumps only that subtree. Hosts without `python3` + `gi.repository.Atspi` are reported as a prerequisite skip, never a pass.

### Window backend note

Keyboard-driven captures on the review host require the X11 backend: launching the client with `WINIT_UNIX_BACKEND=x11` gives the clay Frame AT-SPI `active`+`focused` states and lets xdg-desktop-portal key delivery reach the editor. Wayland sessions deliver portal keys only to native dialogs, not clay windows. Multi-stroke chords (`Ctrl+X Ctrl+P`) cannot be delivered through the portal (each combo arrives as press+release and the pending-chord timeout is ~1.5 s); the Command Centre capture instead re-binds `controlCenter.open` to a single chord, or reuses a prior live capture of the same build.

### Plan 089 Wayland platform smoke

`tests/live_atspi_smoke.rs::live_multi_window_scale_smoke` is an ignored,
environment-gated check (`CLAY_LIVE_WINDOW_SMOKE=1`) rather than part of the
ordinary suite. It starts one isolated server and two real clients, applies a
complete three-profile `setTypography` configuration, and probes AT-SPI frame
identity using application PIDs because separate Clay processes can expose the
same per-application object path. `bounds` mode reads screen-coordinate
component extents; the test requires two distinct frames, positive extents
within a logical-900×600-derived envelope, and two bounded large-type status
bars. `masonry_shell::tests::rescale_event_recomputes_logical_bounds_from_physical_size`
provides deterministic `Rescale(2.0)`/1800×1200-to-900×600 coverage.

Manual completion, Command Centre, settings, file-browser, multi-tab/multi-pane,
narrow/wide, DPI, and native-dialog flows must first pass
`computer-use-linux doctor` with safe window query/focus backends. If
`can_query_windows` or `can_focus_windows` is false, run
`computer-use-linux setup-window-targeting` and complete the requested shell
reload; never use blind portal coordinates or unscoped chords. On the current
GNOME host the exact blocker is
`org.freedesktop.DBus.Error.ServiceUnknown`, so those states remain
`UNRESOLVED` until targeting or a semantic no-focus action path exists.

## Review Artifacts

Captured runs live under `code-reviews/screenshots/<run>/` with `review.status` per state plus a `review-log.md` state table (screenshot/AT-SPI result/verdict per state, numbered findings, and an explicit "no screenshot contains secrets or absolute paths" statement). Example evidence trees:

- `code-reviews/screenshots/2026-08-14-plan086-a11y/` — plan 086 accessibility review
- `code-reviews/screenshots/2026-08-14-plan087-ui-foundation/` — plan 087 default/loading/error/recovery/opened-document/completion/empty-completion/command-centre captures

Screenshots are full-desktop portal PNGs; the current host has no imaging
library or pure-stdlib cropper, so inspect the PNG before retaining it and do
not claim it is app-only. `accessibility.txt` is restricted to the Clay
application subtree. PNGs are review evidence, not CI goldens.

## Plan 089 runtime loading/recovery closure

The Plan 088 follow-up exposed two separate startup races: runtime reload itself was working (generation 2 reached the client), but an empty initial `TabRegistrySnapshot` could prematurely finish layout restore before its `TabId` confirmation, dropping the `loading.txt` reopen; and nested SDUI reconciliation reused an unchanged node ID across a kind change, leaving the editor-only bootstrap child in place. The restore gate now waits for confirmation, the fixture server starts in its private workspace, and nested kind changes rebuild. The sidebar viewport supplies bounded width/fill constraints so the published loading panel is visible and accessible.

`code-reviews/screenshots/2026-08-14-plan089-platform-validation/` records `PASS` for `default`, `error`, `loading`, and `recovery`. The loading capture's `runtime-tree.txt` records the published `Loading review` / `Loading workspace…` snapshot; the cropped screenshot shows the label in Clay's SDUI slot and the AT-SPI dump exposes the Server-driven UI region with a distinct fixture document. Recovery shows synchronized `Connection lost` / `Connection: Disconnected` labels. Structural guards are `masonry_editor::tests::runtime_loading_tree_reaches_accessibility_after_document_open`, `driver::restore::tests::restore_completion_waits_for_registry_tab_id`, and `masonry_editor::tests::disconnected_welcome_accessibility_tracks_status_update`.

## Plan 088 modernization review record

Task 8 retained current-build evidence under `code-reviews/screenshots/2026-08-14-plan088-modernization/`: `default`, `error`, `recovery`, `light-default`, `large-typography`, `loading`, `completion`, and `command-centre`, plus comparison-only pre-task artifacts. The non-interactive capture files report `PASS` for the reachable fixture shell, but review findings still matter: the loading fixture exposed the welcome shell instead of the intended loading SDUI tree, and the recovery tree/status showed a stale WelcomeWidget `Connected` label while the pane/status chrome said `Disconnected`. Those are observability/state-sync follow-ups, not visual passes.

Completion and Command Centre remain `UNRESOLVED` in the current run because this GNOME Wayland host has no safe window-list/focus backend: targeted actions cannot map to the Clay window and unscoped portal chords land in the globally focused application. Narrow/wide, live DPI, file-browser, settings, and multi-tab/multi-pane interaction states likewise retain structural evidence but no false visual pass. The harness contract is to preserve the artifact and reason in `review.status`, then rerun once window targeting or a no-focus fixture action path is available.

Plan 088 manual-plan records link these findings to step ranges `L15–L19`, `F38–F41`, `E22–E24`, `K73–K77`, `Q15–Q19`, `S36–S40`, and `T71–T76`; the review is a bounded evidence-producing gate, not a replacement for source/conformance tests or a GPU pixel golden.

## Phase 28.7 P2 visual and accessibility recapture (2026-08-21)

The P2 review ran the UI preflight again with `npx ui-skills start`, selected
`rams/rams` from the `accessibility` category, then called
`computer-use-linux_get_app_state` before any interaction. Fresh static
fixtures passed and were inspected under
`code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/`:
`default`, `loading`, `error`, `recovery`, and `large-typography`.

Interactive fixtures are intentionally recorded as `UNRESOLVED` when their
state was not reached: the completion and Command Centre triggers had no
keyboard backend, and the Rust analyzer fixture did not produce a non-empty
inlay set after an AT-SPI `SetValue` edit. Fold collapse, link hover/focus/
activate, comment/list/heading mutation, inlay toggle, and narrow/wide live
resize therefore retain structural/security evidence rather than false visual
passes. `review-log.md` records the exact `computer-use-linux_doctor` blocker
(`/dev/uinput` denied, no xdotool/ydotool, Wayland portal input unavailable)
and all per-state verdicts.

This review also records a current accessibility ceiling: the custom editor
has no separate AT-SPI Link node or link-purpose announcement. Links retain
underline/rest styling, caret/keyboard activation, safe target planning, and
HTTP/traversal denial; adding native link semantics later must remain a generic
Clay-owned AccessKit surface, not a package callback or client-JavaScript path.

## Invariants and Constraints

- Every run uses a fresh mode-700 root: no ambient `~/.config/clay`, no default socket, no ambient server/config.
- Server and client are killed and the root removed on every exit path (timeout included).
- A missing accessibility bus or unreachable state yields `UNRESOLVED` with a reason (exit 2), never `PASS`.
- Clay AT-SPI dumps must contain no document secrets or host paths; fixture
  workspaces hold only review files. Full-desktop PNGs are inspected before
  retention because unrelated desktop context can be visible.
- The drift guard `plan087_ui_review_harness_command_and_prerequisites_are_documented` in `tests/manual_smoke_docs.rs` (protocol suite) re-asserts the documented commands, the eight fixture `init.js` files, and script safety markers, and forbids `cargo run -- smoke-gui` as a review substitute.

## Related

- [docs/development/launch-and-gui-smoke.md](../../development/launch-and-gui-smoke.md) — harness reference (fixture/state/capture table, `WINDOW_WIDTH`/`WINDOW_HEIGHT` constants, UNRESOLVED semantics)
- [docs/development/ui-observability.md](../../development/ui-observability.md) — observability entry point
- [Masonry Shell Runtime](masonry-shell.md) — shell/chrome hosting the states the harness captures
- [Pane Document Views](pane-document-views.md) — welcome entry state and completion projection
- [Centered Command Centre Surface](centered-command-centre-surface.md) — the centered modal the harness captures
- [test-plan/index.md](../../test-plan/index.md) — manual step IDs per state (L12–L14, F32–F37, E16–E21, K69–K72, Q11–Q14, S33–S35)
