# Clay Manual Test Plan — Index

Manual verification guide for the whole Clay application. Automated suites
(`cargo test`, 4 declared suites) gate every change; these documents cover
what only a human at a real keyboard/screen can verify: rendering, blink,
ligature glyphs, IME feel, native dialogs, focus, timing.

## How to use this plan

1. Build once: `cargo build` (or `cargo run` which builds on demand).
2. Pick the module file(s) relevant to what you changed — the table below.
3. Each module file is self-contained: setup, numbered steps with expected
   results, negative checks, and known ceilings that are NOT bugs.
4. Record results inline (copy the table or keep notes). Failures that match
   a file's "known ceilings" section are expected behavior, not defects.
5. Every plan document that changes user-visible behavior must update the
   affected module file(s) and this index (enforced by the create-plan skill
   manual-test-plan task).

## Prerequisites (all modules)

- Linux host (primary platform), Rust toolchain, `cargo`.
- Optional: `~/.config/clay/` config tree (canonical example: `examples/` — copy with `cp -r examples/. ~/.config/clay/`).
- Scratch workspace: `mkdir -p /tmp/clay-manual` with sample files (each
  module file lists the files it needs, or points at a shared setup).
- Font for ligature checks: Fira Code (`FiraCode Nerd Font Mono` works).

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

Current desktop launch is Tauri v2 + React: `clay` and `clay client` launch
`clay-desktop`; `clay server` remains standalone. The dated review artifact is
`code-reviews/screenshots/2026-08-24-tauri-react-parity/`.

The review retains 20 app-only CDP screenshots and paired AX snapshots at
1440×900 and 780×900 for editor, intelligence, package UI, settings,
Command Centre (active/empty), Path Browser, Chat, splits, and combined
loading/empty/error states. Real Tauri AT-SPI dumps cover welcome, opened
editor, tabs/splits, and Chat. Static visual and rest-state accessibility checks
pass. Keyboard-only completion, command/path activation, native dialog,
settings, and tab/pane interaction remain explicitly `UNRESOLVED`: this host
has denied `/dev/uinput`, no `xdotool`/`ydotool`, and no Wayland portal path that
can target Clay. Full-desktop portal screenshots with unrelated windows were
removed; no retained screenshot contains host paths or secrets.

Review fixes: editor labels no longer expose an absolute workspace fallback,
and the shell connection status is a polite live region. The only low-priority
follow-up is the unselected Settings theme control's repeated `Theme Theme`
accessible name. See the per-module records below and
`docs/development/accessibility.md` for the current role contract.

## Plan 097 manual-test-plan execution record (2026-08-24, post-cutover)

Executed against the current `target/debug/clay` / `clay-desktop` Tauri build
via `scripts/capture-ui-review.sh` (isolated config/data/workspace per run):

| Fixture | Result | Evidence/notes |
|---|---|---|
| ui-review-default | PASS | AT-SPI tree exposes `clay-desktop` frame, `Clay workspace`, `Window tabs` page-tab list (selected `Workspace` tab), `Pane 1`, named Open File/Open Folder actions, and status bar |
| ui-review-large-typography | PASS | Large UI typography renders in bounds; controls legible |
| ui-review-command-centre | UNRESOLVED | Interactive state requires a TTY for keyboard capture — documented host ceiling (no `/dev/uinput`, no xdotool/ydotool, no Wayland portal input path) |
| ui-review-error | UNRESOLVED | New finding: the sanitized runtime diagnostic never appears in an AT-SPI name dump because WebKitGTK does not expose static text inside the footer/live region as accessible names or Text-interface content (verified with a targeted AtspiText probe; even the `Connected` status text is invisible to AT-SPI). Diagnostic delivery itself is covered by automated tests (server diagnostic broadcast → workspace-controller handling → `app-shell.tsx` resolvedStatus render). Follow-up: expose footer/live-region text to AT-SPI, then re-enable this step |
| Frontend production build budgets | PASS | shell 160.6/180 kB gzip; total 343.2/400 kB gzip |

Modules 05 (movement/selection), 06 (multi-cursor), and 12 (Windows) received
dated status records below: interactive keyboard steps stay UNRESOLVED on this
host; their logic is pinned by frontend editor tests and CodeMirror built-ins.
Stale native-era step references (deleted Masonry unit tests and wiki deep
references) were replaced with current equivalents in modules 01, 04, 07, 10,
13, and 14 — no existing behavior step was weakened.

## Module map

| # | Module file | Covers | Deep-reference doc |
|---|-------------|--------|-------------------|
| 01 | [Launch and connection](01-launch-and-connection.md) | server/client lifecycle, lease, read-only observer, restart, status line | `docs/development/launch-and-gui-smoke.md` |
| 02 | [Configuration (init.js)](02-configuration-init-js.md) | init.js evaluation, modular loading, diagnostics, live reload, watcher auto-reload, default reload chord, planned-API denial | `docs/reference/clay-js-api/configuration.md`, `examples/` tree, `tests/fixtures/configuration/plan080-manual/` |
| 03 | [Files and workspace](03-files-and-workspace.md) | open/save/reload, dirty state, conflicts, sanitized file-browser/workspace labels, hidden-pane toggle, `Ctrl+O` while hidden, multi-document (incl. pane-scoped switcher, duplicate-open focus routing), Path Browser (24.3): seed fallback, fuzzy filter, descend/ascend/direct jump, invalid-path recovery, file open + duplicate-open focus + active-pane targeting, `Alt+Enter` current-tab workspace load, cancellation, tab-switch/reload dismissal, native-dialog fallback, navigation-no-grant/symlink/cross-tab security checks, centered modal surface/accessibility/containment (24.4) | `docs/development/file-open-save-reload-workflow.md`, `docs/reference/clay-js-api/configuration.md` (Phase 24.3 review) |
| 04 | [Core editing](04-core-editing.md) | typing, undo/redo, clipboard, newline/indent rules, IME preedit, completion projection/ranking, Phase 28 comment/list/heading transforms and inlay toggle, bounded AT-SPI/AccessKit editable-text semantics | `docs/reference/clay-js-api/editor/` command docs, `docs/development/accessibility.md` |
| 05 | [Movement and selection](05-movement-and-selection.md) | word/paragraph/line movement, sticky column, line/word selection, prose vs code | `docs/development/manual-editor-capabilities-test-plan.md` |
| 06 | [Multi-cursor editing](06-multi-cursor.md) | Ctrl+D match selection, column select, add-cursor, cursor undo, escape priority | `docs/development/manual-editor-capabilities-test-plan.md` |
| 07 | [Caret and typography](07-caret-and-typography.md) | caret shape/blink, width, ligature policies per font role, user-owned hierarchy, large/small UI typography and theme contrast | `docs/development/manual-editor-capabilities-test-plan.md` |
| 08 | [Syntax and text objects](08-syntax-and-textobjects.md) | grammar highlighting, textobject/smart-select, engine tiers, advisory degrade, Phase 28 folding ranges, link intent, and inlay overlays | `docs/development/manual-editor-capabilities-test-plan.md`, `docs/reference/primitives/ui-chrome-primitives.md` |
| 09 | [Packages and modes](09-packages-and-modes.md) | package loading, mode classification/activation, settings UI, theme switching, clipped/scrollable package panels, state/disabled/provenance semantics, Phase 27 inspect/preset/one-line load, Phase 28 behavior/keymap/LSP contributions | `docs/development/launch-and-gui-smoke.md`, `docs/reference/packages/creating-packages.md` |
| 10 | [Keybindings and commands](10-keybindings-and-commands.md) | bindKey override, unbind, deny-by-default, execution push channel, Global-scope tab command bindings (22.4), Control Center menu round trip + tab-switch dismissal (24.1), Control Center command execution mode (24.2), Path Browser keybinding surface (24.3), centered modal surface/accessibility/input containment (24.4), sequence chords (24.5), and Phase 28 client-command aliases/package keymaps | `docs/development/manual-editor-capabilities-test-plan.md`, `docs/reference/primitives/shell-layout-strategy.md`, `docs/reference/clay-js-api/keybindings/bind-key.md` |
| 11 | [Performance](11-performance.md) | large files, scroll/type latency, parse feel, window-model budgets (22.6: pane paint / tab switch / decoration aggregate), centered Command Centre rendering feel (24.4: one panel + scrim, width clamping, no duplicate overlays, no blur jank), Command Centre open/filter feel + chord pending feel (24.5 advisory budgets), completion popup feel/caps (Plan 087), Plan 088 responsive/high-DPI/typography geometry, and Phase 28 fold/link/inlay/ranking budgets | `docs/development/performance.md` |
| 12 | [Platform: Windows](12-platform-windows.md) | MSVC toolchain, named pipes, native dialogs | `docs/development/windows.md` |
| 13 | [Window splits](13-window-splits.md) | split/close/add-equal/move/resize panes, pane focus policies, per-pane document views + concurrent modes (22.2), Phase 22.8 per-tab multi-document isolation, shell keybinding overrides (per active tab since 22.3), direction-named split aliases (22.7), per-tab persistence cross-check (22.5), pane a11y roles + split/pane announcements (22.6) | `docs/reference/primitives/shell-layout-strategy.md`, `docs/development/accessibility.md` |
| 14 | [Tabs (independent client views)](14-tabs.md) | tab bar, selected-root tab binding and per-tab workspace/document isolation (22.8), open/switch/close tabs, per-tab connections + split trees + documents, edit isolation, dirty-guarded close, keyboard tab management incl. numbered activate/move + confirm close (22.4), reconnect + restart reclaim, window-state persistence incl. restore/failure/hostile-file steps (22.5), tab a11y (TabList/Tab roles, activate/create/close announcements) + cross-tab grant isolation/denial checks (22.6/22.8), tab-bar overflow scroll (22.7), active-typography geometry and sanitized tab labels (Plan 088), single-tab match-today | `docs/reference/primitives/shell-layout-strategy.md`, `docs/wiki/modules/react-tabs-and-splits.md`, `docs/wiki/modules/tabs-and-clients.md`, `docs/development/accessibility.md` |

## Coverage matrix (what to run when)

| Change touches | Minimum manual modules |
|----------------|------------------------|
| Client rendering / surface / paint / transient overlays | 01, 04, 07, 10, 11 |
| Editor movement/selection/caret primitives | 05, 06, 07, 10 |
| Typography / font features | 07 |
| Protocol / IPC / connection | 01, 03, 04 |
| Configuration surface / init.js APIs | 02, 10 + the module of the feature configured |
| Syntax / grammar / decorations | 08 |
| Package loading / modes / trust boundary | 09, 02 |
| File IO / save / dialogs | 03 |
| Keybinding routing / commands | 10, 05, 11 |
| Shell layout / panes / splits / pane focus / split aliases | 13, 10, 01 |
| Tabs / selected-root workspace binding / cross-tab authority / tab bar / keyboard tab chords / multi-connection / reconnect / window-state persistence / tab-bar overflow scroll | 14, 13, 03, 01 |
| Pane/tab/transient-menu accessibility (roles, names, announcements) / cross-tab isolation | 10, 13, 14, 03 |
| Pane document views / concurrent modes / duplicate-open routing | 13, 03, 09 |
| Anything user-visible | 01 always (launch gate) |
| Welcome entry state / completion projection / centered Command Centre / review harness (Plan 087) | 01 (L12–L14), 03 (F32–F37), 04 (E16–E21), 10 (K69–K72), 11 (Q11–Q14), 13 (S33–S35) |
| Plan 088 shell/theme/package modernization and responsive layout | 01 (L15–L19), 02 (C20–C24), 03 (F38–F41), 04 (E22–E24), 07 (T14–T17), 09 (P16–P21), 10 (K73–K77), 11 (Q15–Q19), 13 (S36–S40), 14 (T71–T76) |
| Plan 089 validation, performance, timeout diagnostics, and multi-window/scale/Wayland platform checks | 01 (L18–L22), 04 (E22–E24), 07 (T15/T18–T19), 09 (P16–P21), 10 (K73–K77), 11 (Q15–Q19), 13 (S36–S42), 14 (T71–T76) |
| Phase 26 rendering quality (theme color/background/scale axes, heading size ladder, wrap policies, editor chrome, decoration backgrounds, dirty-pane close fix) | 04 (E25–E27), 07 (T20–T27), 08 (S16–S19), 09 (P22–P24), 11 (Q20–Q23), 13 (S43–S46) |
| Phase 28 editor commands/intelligence (comment/list/heading transforms, package keymaps, folding, links, inlays, completion ranking, editable-text accessibility) | 01 launch gate, 04 (E28–E36), 08 (S21–S32), 09 (P29–P31), 10 (K78–K83), 11 (Q24–Q27) |
| Plan 097 Phase 8 React SDUI/package UI and trust domains | 01 launch gate, 09 (P32–P36), 11 (Q28–Q30) |
| Plan 097 Phase 9 React Command Centre, paths, configuration, settings, and desktop workflows | 01 launch gate, 02 (C26–C29), 03 (F42–F47), 09 (P37–P42), 10 (K85–K91), 11 (Q31–Q33) |
| Plan 097 Phase 5 CodeMirror editing + optimistic document sync | 04 (E-series), 03 (open/save/reload), 11 (type latency) |
| Plan 097 Phase 6 panes/splits/tabs/per-tab workspaces/persistence | 13, 14, 03 (workspace roots), 01 (reconnect) |
| Plan 097 Phase 7 editor interaction/rendering/completions/language intelligence | 04, 05, 06, 07, 08, 11 |
| Plan 097 Phase 10 AG-UI chat over Tauri channels | 09 (@clay/chat surface), 10 (chat intents), 11 (stream feel) |
| Plan 097 Phase 11 release hardening/packaging/updates/security | 01 (launch identity), 11 (budgets), 12 (platform policy) |
| Plan 097 Phase 12 parity certification/cutover/native removal | 01, 13, 14 + full regression pass of modules above |

## Plan 097 Phase 9 Linux execution record (2026-08-23)

React fixture review covered active/empty/narrow Command Centre, labelled
search/listbox/options/live count, settings collapsed/expanded/narrow/invalid
states, and token-only containment. Evidence:
`code-reviews/screenshots/2026-08-23-tauri-react-phase9/`. C26–C29,
F42–F47, P37–P42, and K85–K91 record exact automated/live boundaries.
`computer-use-linux_get_app_state` ran first; AT-SPI worked but development
keyboard input was unavailable, so CDP supplied DOM interaction/accessibility
evidence and native picker selection remains explicitly blocked. No existing
step was removed or weakened.

## Plan 097 Phase 8 Linux execution record (2026-08-23)

React fixture review covered package slots, SDUI editor composition, settings controls, status contribution, dropdown interaction, narrow/wide layout, and large typography. Evidence: `code-reviews/screenshots/2026-08-23-tauri-react-phase8/`. P32–P36 and Q28–Q30 record exact automated/live boundaries. CDP provided the rendered accessibility tree because AT-SPI exposed only the Chrome frame and compositor targeting omitted Chrome-for-Testing. No existing step was removed or weakened.

## Plan 088 task 12 Linux execution record (2026-08-15)

Executed against the current `cargo build` on real Linux/GNOME Wayland. Mandatory UI preflight for this task was the UI guidance current at execution time; selected review skills were `wshobson/wcag-audit-patterns` (accessibility/testing) and `vercel-labs/web-design-guidelines` (visual/accessibility), applied to Clay's token/AT-SPI context. `computer-use-linux get_app_state` and `doctor` ran before launch; AT-SPI/screenshot capture works, but `can_query_windows=false` and `can_focus_windows=false`, so targeted keyboard, resize, and native-dialog actions are not claimed as passes.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 L15–L17, 20; 03 F39/F41 | PASS | Current Clay-only review artifact `code-reviews/screenshots/2026-08-15-plan088-task12-manual/default/` has `review.status PASS`, 900×600 logical metadata, named welcome actions/status, and no absolute path; retained error/light/large captures add dark/light/error/typography coverage |
| 01 L18, L19; 04 E23; 10 K74/K75; 13 S39; 14 T75 | FAIL/UNRESOLVED follow-ups | Recovery capture has stale WelcomeWidget Connected status; loading capture renders welcome instead of intended loading tree; interactive completion/Command Centre/split/tab states cannot be driven safely. Findings remain explicit in module records |
| 02 C20–C24; 07 T14/T17 | PASS automated/headless; manual reload partial | Canonical example test, Node checks, typography validation, and atomic rejection tests pass; live reload input is host-blocked |
| 03 F14/F38/F40; 09 P16–P21; 10 K76/K77 | PASS structural / blocked live | Sanitization, clipping, package/theme contract, typed-token validation, modal Escape, disabled/status, stale-session, and authority tests pass; settings/native/package interaction cannot be focused |
| 11 Q15–Q19; 13 S36–S38; 14 T71–T74/T76 | PASS advisory/structural; blocked visual extremes | Window benchmarks completed; responsive/high-DPI/tab-overflow/layout tests pass; live window resize and multi-tab targeting unavailable |

The full step additions and per-module evidence are recorded in modules [01](01-launch-and-connection.md), [02](02-configuration-init-js.md), [03](03-files-and-workspace.md), [04](04-core-editing.md), [07](07-caret-and-typography.md), [09](09-packages-and-modes.md), [10](10-keybindings-and-commands.md), [11](11-performance.md), [13](13-window-splits.md), and [14](14-tabs.md). No existing step was deleted or weakened. `P1-087-UI-1`, the recovery WelcomeWidget status mismatch, and loading-fixture observability remain explicit follow-ups.

## Plan 089 task 9 Linux execution record (2026-08-17)

Real Linux/GNOME Wayland execution used `cargo build`, the isolated mode-700 review harness, xdg-desktop-portal PNG capture, Python GI/AT-SPI dumps, and the now-active GNOME Shell extension for window targeting (`can_query_windows=true`, `can_focus_windows=true`).

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 L18; 14 T75 | PASS | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/recovery/` shows `Connection lost` / `Connection: Disconnected` consistently after the `request_welcome_render` fix; the Plan 088 P1 stale WelcomeWidget Connected status is resolved |
| 01 L19 | PASS (delivered-RuntimeStateSnapshot evidence) | `loading/` with `runtime-tree.txt` confirms the published loading SDUI tree was delivered via `RuntimeStateSnapshot`; the restore-gate fix and kind-changed reconcile fix ensure the tree reaches the accessibility layer |
| 01 L20–L22; 07 T18–T19; 13 S41–S42 | PASS live/headless | `CLAY_LIVE_WINDOW_SMOKE=1` multi-window smoke test launched two real Clay clients; AT-SPI exposed two PID-separated frames with positive bounds and scale factors within 0.5–4.0; `rescale_event_recomputes_logical_bounds_from_physical_size` passes; responsive narrow/wide captures show the welcome card and status bar within bounds |
| 04 E22; 10 K74 | PASS live | `completion/` capture shows the bounded completion popup with 44 children, `as` selected, no rows exceeding the visible surface; P1-087-UI-1 containment is visually verified |
| 10 K75 | UNRESOLVED live / PASS structural | Command Centre remains UNRESOLVED because `Ctrl+Alt+P` is consumed by GNOME before reaching Clay; structural clipping/single-scrim/modal-role tests pass |
| 10 K77 | PASS automated | Plan 089 added `compact_generated_frame_mutations_fail_closed_without_panicking`, `editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions`, and `generated_menu_intent_ordering_preserves_lifecycle_and_authority` |
| 11 Q15 | PASS advisory run + triage | Plan 089 Criterion triage classified every group as machine variance except centered_overlay as benchmark instability; no reproducible implementation regression; no budget raised |
| 09 P16–P21 | PASS structural / NOT RUN package-panel visually | Plan 089 did not add new package features; settings/package panels remain unrendered because `settings.open` does not persist or make the panel visible |

## Plan 090 task 11 Linux execution record (2026-08-17)

Plan 090 is a responsibility-preserving refactor: existing server/runtime,
editor/shell, package, and app-driver code moved into private modules without
changing user-visible behavior, layout, labels, commands, keybindings, or
platform contracts. No new numbered steps or coverage-matrix entries were
needed; existing module records remain the behavioral baseline. Manual parity
was checked against the current Linux debug build, with direct interaction
steps not duplicated where the plan made no user-facing change. The approved
Plan 090 visual-review waiver remains separate and unchanged.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 L12–L19; 02 C20–C24 | PASS current-build parity | `cargo build`; `node --check` on all three example files; canonical configuration fixture/test; `scripts/capture-ui-review.sh` default/loading/error/recovery/large-typography fixtures all returned `PASS`. Current AT-SPI trees expose the Clay shell, welcome actions, Connected/Editable status, delivered loading snapshot, runtime error, and Disconnected recovery state. |
| 03 F14/F38–F41; 04 E22–E24; 09 P16–P21; 10 K73–K77; 13 S36–S42; 14 T71–T76 | N/A for new manual rerun; parity retained | No user-facing behavior changed in these modules, so no new steps were added or weakened. Existing Plan 089 Linux records remain the latest direct interaction evidence; current `cargo test --all-targets --quiet` passed all editor/protocol/runtime/security suites and all benchmark harness cases. |
| 11 Q15–Q19 | PASS advisory / no blocking budget failure | Current `window_baselines` and `editor_baselines` Criterion runs completed. Small-sample comparisons reported host/baseline variance, while absolute measurements stayed within the documented budgets; per module 11, Criterion comparisons are advisory and not a shared-runner pass/fail gate. |
| Security negative checks | PASS automated; live probe blocked by host | Current all-target test run: security 130 passed, 2 ignored; package/runtime/file/workspace/modal/visibility denial checks remained green. The standalone live AT-SPI smoke probe timed out without discovering Clay on this host, while the capture harness found Clay and produced valid accessibility trees; no source failure is inferred, and the prior Plan 089 live pass remains retained. |

No `test-plan` module instructions were changed, deleted, or weakened. The
coverage matrix is unchanged. Temporary current-run harness output was kept
under `/tmp/plan090-manual/`; no new screenshot artifact was retained because
Plan 090's visual review is N/A by approved scope exception.

## Plan 087 task 11 Linux execution record (2026-08-15)

Real Linux (X11-backend client on the review host) execution used the isolated mode-700 root + `ui-review-completion` fixture init.js with an empty-tab launch. The welcome entry state, the native Open File dialog flow, and the live completion popup were driven through AT-SPI/portal and verified in the accessible tree; client and server stayed alive throughout.

- **PASS:** welcome entry state (module [01](01-launch-and-connection.md#linux-execution-record-plan-087-task-11-2026-08-15)); Open File → native dialog → `review.md` opened with sanitized basenames (module [03](03-files-and-workspace.md#linux-execution-record-plan-087-task-11-2026-08-15)); completion popup 480×340 at caret with 16 items, 8 visible rows, selected row, Escape dismissal, and empty-result dismissal without a blocking panel (module [04](04-core-editing.md#linux-execution-record-plan-087-task-11-2026-08-15)); fixture `completion.trigger` binding and Command Centre non-regression via the plan's task-7 live capture, same build (module [10](10-keybindings-and-commands.md#linux-execution-record-plan-087-task-11-2026-08-15)); completion caps and advisory benches (module [11](11-performance.md#linux-execution-record-plan-087-task-11-2026-08-15)); welcome-return on pane close (module [13](13-window-splits.md#linux-execution-record-plan-087-task-11-2026-08-15)).
- **BLOCKED (host, not a false pass):** this session's xdg-desktop-portal keyboard delivery could not hold Ctrl across the two strokes of `Ctrl+X Ctrl+P`, so the Command Centre/split re-runs were not repeated in this instance; task-7 live captures and automated tests cover those surfaces. `P1-087-UI-1` (popup rows painting below the shell) remains a tracked follow-up.

## Plan 086 task 11 Linux execution record (2026-08-14)

Real Linux/Wayland execution used isolated `clay server <temp-socket>` and `clay client <temp-socket>` processes with mode-700 temporary HOME/XDG roots, a private socket, and a v2 two-tab/two-pane layout. The live AT-SPI tree was queryable and showed deterministic TabList/Tab, pane, status, menu, and announcement nodes. Representative stable IDs were shell TabList `14987979559889014273`, announcement `14987979559889014274`, cards `14987979559889014276/14277`, and Control Center status/items beginning `14987979559889054209`.

- **PASS:** representative launch/status, restored multi-document panes, Control Center open/filter/cancel, split creation/clean close, tab selection/close, bounded announcements, stable virtual object paths, and no-path/ambient-config negative checks. Module-specific records: [01](01-launch-and-connection.md#linux-execution-record-plan-086-task-11-2026-08-14), [03](03-files-and-workspace.md#linux-execution-record-plan-086-task-11-2026-08-14), [10](10-keybindings-and-commands.md#linux-execution-record-plan-086-task-11-2026-08-14), [13](13-window-splits.md#linux-execution-record-plan-086-task-11-2026-08-14), [14](14-tabs.md#linux-execution-record-plan-086-task-11-2026-08-14).
- **FAIL/BLOCKER:** dirty active-pane close crashed the client with `Focused ID #4 is not in the node list` in `accesskit_consumer`; isolated server survived. Evidence is retained under `code-reviews/screenshots/2026-08-14-plan086-a11y/manual-dirty-pane-close-crash.log` and is not a false pass.
- **BLOCKED:** native dialog selection, observer/restart/local-fallback keyboard flows, and full quit/relaunch persistence were not manually re-run because this host's window-targeting/portal backend cannot safely target Clay controls; automated coverage remains separate.

## Conventions

- Steps are numbered `<module><step>` (e.g. `E3`) so failures can cite them.
- "Expected" columns describe the product contract; visual judgments
  (smoothness, glyph shape, blink rhythm) are part of the check.
- Restart = `cargo run` again; live reload = settings appearance switch
  (module 02) unless stated otherwise.

## Phase 26 Linux execution record (2026-08-19)

Executed against the current `cargo build` on real Linux/GNOME Wayland.
Rendering evidence: fresh captures with the current build
(`code-reviews/screenshots/2026-08-19-phase26-manual-test-plan/` — rust,
markdown, long-line fixtures, all `review.status=PASS`) plus the 17-capture
post-implementation visual review
(`code-reviews/screenshots/2026-08-18-phase26-review/`). Interactive
keyboard delivery is partial: single keys reach the app (typed input made a
document dirty live), but modifier chords (`Ctrl+Alt+W`) and scroll are
host-blocked (portal limitation — review-log V9), so dynamic steps are
covered by the automated suites named in each module record.

| Modules/steps | Result | Evidence |
|---|---|---|
| 07 T20–T22, T25 (dark/gruvbox), T26; 08 S16/S17; 09 P22/P24; 11 Q20/Q21 | PASS live (static states) | Phase 26 review captures: heading ladder + prose column wrap (markdown-*), wrap-none long-line clip (rust-longline-default), gutter/active-line/indent-guides/bracket-match on code (rust-*), chrome off on prose (markdown-*), quote/fence backgrounds + distinct rich vocabulary across 4 themes |
| 07 T23/T24/T27; 08 S18/S19; 04 E25–E27; 11 Q22/Q23 | PASS automated / NOT RUN live | `set_editor_layout_*`, `user_wrap_override_beats_manifest`, `column_wrap_is_narrower_than_viewport`, `search_match_and_quote_backgrounds_join_style_runs`, `style_run_backgrounds_paint_before_glyphs`, `size_scale_ladder_descends_headings_and_clamps_theme_overrides`, theme parser validation, `tests/theme_packages.rs`, incremental parse continuity tests, `editor_baselines` + 16 ms envelope guards; live reload/typing/scroll input is host-blocked |
| 13 S43/S44 (dirty-pane close fix) | PASS automated regression; live partial | `dirty_focused_pane_menu_and_discard_keep_consumer_focus_live` + `dirty_pane_close_rejection_and_discarded_removal_keep_focus_consumer_safe` exercise the exact Plan 086 crash path (menu apply → DirtyDocument → discard → close) with the consumer focus live at every step; the Plan 086 `accesskit_consumer` panic no longer reproduces. Live attempt: typed input dirtied a real document (doc v2) with the client alive and the AT-SPI tree intact; the `Ctrl+Alt+W` chord itself is host-blocked (single-key delivery only) |
| 13 S45/S46 (per-pane chrome) | PASS live (single-pane) / structural (multi-pane) | rust-* vs markdown-* captures show per-mode chrome; pane-scoped paint tests + per-pane decoration aggregate guard cover multi-pane isolation |
| 07 T25 light-theme gutter digit | DEFECT — V4 | `*-modus-operandi/` code captures: current-line gutter digit invisible (`gutterFgActive` 0xf4f1ff vs light `lineHighlight`/panel). Tracked in `code-reviews/screenshots/2026-08-18-phase26-review/review-log.md` V4; fix = light themes define `gutterFgActive` or theme-aware default. Not a blocker for the other Phase 26 steps |

## Phase 28 manual test-plan execution record (2026-08-20)

Executed against a fresh `cargo build --bin clay` on real Linux/GNOME
Wayland. UI preflight for this task: the UI guidance current at execution time; categories
`accessibility` and `testing` inspected; selected
`jakubkrehel/better-accessibility`. AT-SPI and xdg-desktop-portal captures were
available. Full evidence: `code-reviews/screenshots/2026-08-20-phase28-manual/manual-test-plan.md`.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 launch gate; default/error/recovery; large typography | PASS | Fresh `code-reviews/screenshots/2026-08-20-phase28-manual/{default,error,recovery,large-typography}/` captures expose named controls, bounded status/diagnostic text, and Connected/Disconnected state |
| 04 E28–E32; 10 K78/K80/K82/K83 | UNRESOLVED live; PASS structural | Editor Entry reports `supports_editable_text=false`; no keyboard mutation or preview/comment round-trip claim. Transform, alias, preview registration, routing, and menu-consumption tests pass |
| 04 E33; 11 Q24 | PARTIAL live; PASS automated | Completion rest popup captured; `hel` had no bundled match, so visual prefix order is not claimed. Scorer/recency/cap/hot-path tests pass |
| 08 S21–S24; 11 Q25 | PASS rest / UNRESOLVED collapse feel; PASS automated | Rust fold chevrons captured; compositor targeting blocked repeatable collapse/scroll. Fold visibility and permission/budget tests pass |
| 08 S25–S28; 11 Q27 | PASS rest / UNRESOLVED interaction; PASS automated/security | Markdown link styling captured; pointer targeting blocked hover/activation. Target planning, traversal/HTTP denial, decoration cap, and no-network tests pass |
| 08 S29–S32; 09 P30; 11 Q26 | UNRESOLVED live; PASS worker/bridge structural | P1 repaired `lsp-shared` session options, analyzer workspace-root context, and decoration viewport bytes. Fresh GUI reaches Rust bridge with no `analysis.worker_failed` and emits an inlay set, but first provider response is empty during rust-analyzer warm-up; keyboard backend is unavailable, so visible/toggled-off states remain unresolved under `code-reviews/screenshots/2026-08-20-phase28.7-followups/` |
| 09 P29/P31; 10 K79/K81/K83 | PASS automated / NOT RUN live | Package manifest/keymap parser, Markdown preview registration, closed command backing, malformed chord, permission, and activation tests pass; editable focus prevented false live claims |

No existing manual step was deleted or weakened. Unresolved live rows retain
explicit host/tooling blockers and remain linked to Plan 095 follow-ups.

## Phase 28.7 P2 visual, interaction, and accessibility recapture (2026-08-21)

Executed against the current `target/debug/clay` on real Linux/GNOME Wayland.
The UI preflight ran for this task: the UI guidance current at execution time, category
`accessibility`, selected `rams/rams`, then
`computer-use-linux_get_app_state` and `computer-use-linux_doctor` ran before
interaction. Static fixture states passed; the desktop reports no development
keyboard backend. Full evidence:
`code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/review-log.md`.

| Modules/steps | Result | Evidence |
|---|---|---|
| 01 launch gate; default/error/loading/recovery/large typography | PASS | Fresh `default/`, `loading/`, `error/`, `recovery/`, and `large-typography/` captures have `review.status=PASS`; AT-SPI dumps expose named controls, bounded panel/status text, recovery menu selection, and Connected/Disconnected state. |
| 04 E28–E36; 08 S21–S32; 09 P29–P31; 10 K78–K83; 11 Q24–Q27 | Mixed live; PASS structural | Completion/Command Centre triggers, fold/link/inlay interactions, comment/list/heading/preview keyboard mutation, and live resize remain explicit `UNRESOLVED`; retained P1 EditableText interface evidence remains valid, with physical keyboard mutation host-blocked. No false visual interaction pass is claimed. |
| Automated companions for E/S/P/K/Q rows | PASS | 39 editor invariants, 24 performance budgets, 19 performance protocol tests, 5 decoration-intent authority tests, focused transform/fold/inlay/completion tests, and prior full Linux suite pass. |

No step was deleted or weakened. Static states are refreshed; unresolved live
rows remain explicit and linked to module records and Plan 094 evidence.
