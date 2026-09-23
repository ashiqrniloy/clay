# 10 — Keybindings and Commands

`bindKey`/`unbindKey` overrides, deny-by-default validation, command routing,
the `editor-control` execution push channel (`clientExecuteEditorCommand`,
protocol v8), the Global-scope tab command bindings (Phase 22.4), and
sequence chords (Phase 24.5: multi-stroke bind, pending/timeout/cancel, chord
defaults, prefix-collision rejection). Deep
reference: `docs/development/manual-editor-capabilities-test-plan.md`
(sections G/H) + `docs/reference/clay-js-api/shell/client-tab-*.md`.

## Setup

init.js:

```js
import { bindKey, unbindKey } from "clay:keybindings";
bindKey("Ctrl+B", "editor.clientMoveCursor.prevWordStart", { scope: "editor" });
```

## Override and validation

| # | Action | Expected |
|---|--------|----------|
| K1 | `Ctrl+B` in any file | Moves one word back (init.js binding beats nothing; direction-specific IDs are bindable) |
| K2 | Add a second `bindKey("Ctrl+B", "editor.clientMoveCursor.nextWordStart", …)`, reload | Last binding wins — now moves forward |
| K3 | `unbindKey("Ctrl+B", { scope: "editor" })`, reload | Default/no binding restored |
| K4 | `bindKey("Ctrl+G", "application.quit", …)` | Rejected — non-editor/undeclared command IDs deny-by-default; diagnostic names it |
| K5 | `bindKey("Ctrl+Q Ctrl+W", "editor.clientMoveCursor.nextWordStart", …)` (multi-stroke) | Accepted — space-separated stroke sequence (Phase 24.5); see the Sequence chords section below |
| K6 | Bind a textobject ID (`editor.clientSelectTextobject.class.around.current`) | Accepted — auto-declared on first bind; works in grammar files |

## Sequence chords (Phase 24.5)

Multi-stroke chords are space-separated stroke lists (`"Ctrl+X Ctrl+P"`,
`"g g"`); single-stroke chords remain the fast path. The first stroke of a
sequence is consumed without any other effect: a pending chord is held until
the sequence completes, times out (server-owned constant
`KEY_CHORD_PENDING_TIMEOUT_MS`, ~1.5 s, cancelled and re-checked on the next
keystroke), or mismatches — on mismatch the chord cancels and the key
re-evaluates fresh, so typing is never eaten. A binding whose sequence is a
strict prefix of another binding in the same scope is rejected at bind time.
Deep reference: `docs/reference/clay-js-api/keybindings/bind-key.md`
(Multi-stroke chords), `examples/init.js` (key-format section),
`docs/development/performance.md` (Phase 24.5).

init.js:

```js
import { bindKey } from "clay:keybindings";
bindKey("Ctrl+Q Ctrl+W", "editor.clientMoveCursor.nextWordStart", { scope: "editor" });
```

| # | Action | Expected |
|---|--------|----------|
| K60 | With the init.js above, reload; press `Ctrl+Q` then `Ctrl+W` | First stroke consumes nothing (no text, no caret move); second stroke dispatches — caret moves one word forward (automated: `route_key_sequence_tracks_a_two_stroke_chord`, `editor_pending_chord_consumes_strokes_and_dispatches_on_completion`) |
| K61 | Press only `Ctrl+Q` and wait | Nothing happens while pending — no text inserted, no command runs, no visible delay on the stroke itself |
| K62 | Press `Ctrl+Q` then a plain `x` (mismatch) | Chord cancels; `x` routes normally and inserts into the document — no eaten typing (automated: `route_key_sequence_mismatch_clears_the_chord`, `editor_abandoned_chord_does_not_eat_the_next_key`) |
| K63 | Press `Ctrl+Q`, wait ~2 s (past the server-owned ~1.5 s timeout), then press `Ctrl+W` | Stale chord cancelled; `Ctrl+W` routes fresh — no command runs (automated: `editor_stale_pending_chord_cancels_on_the_next_key`) |
| K64 | Fresh profile (no init.js), press `Ctrl+X Ctrl+P` | The persistent agent lane toggles (plan 124; the composer-anchored palette moved to `Ctrl+X Ctrl+O`) — the shipped chord default routes through the inert behavior manifest (automated: `default_keymaps_are_prefix_collision_free`, `client_routes_control_center_open_default_binding_as_server_intent`) |
| K65 | Fresh profile, press `Ctrl+X Ctrl+F` | Path Browser opens (browse behavior in module 03, F17–F29) |
| K66 | init.js: `bindKey("g g", "workspace.refresh", { scope: "global" })` then `bindKey("g", "controlCenter.open", { scope: "global" })`, reload | Second bind REJECTED — a strict prefix of a same-scope binding is ambiguous (`keybindings.bind_failed`, diagnostic names the colliding rule; automated: `manifest_rejects_prefix_collisions_within_a_context`, `configuration_bind_key_prefix_collision_is_rejected`) |
| K67 | With the chord bindings installed, re-run K1–K3 (single-stroke `Ctrl+B` bind/unbind) | Single-stroke chords stay on the fast path — immediate dispatch, no pending delay (automated: `route_key_sequence_matches_single_stroke_without_regression`) |
| K68 | Negative: from package context, bind a chord to an unregistered command (`bindKey("Ctrl+Q Ctrl+W", "application.quit", …)`) | Rejected deny-by-default exactly like single-stroke — chords grant no new binding authority (automated: `unknown_command_binding_is_rejected`) |

## Default bindings sanity (must exist without any init.js)

| # | Key | Expected |
|---|-----|----------|
| K7 | Arrows, `Home`/`End`, `Ctrl+Home`/`Ctrl+End` | Basic movement (module 05) |
| K8 | `Ctrl+Left`/`Ctrl+Right`, `Ctrl+Up`/`Ctrl+Down` | Word / paragraph movement |
| K9 | `Ctrl+D`, `Ctrl+Shift+L`, `Ctrl+Alt+Up`/`Down`, `Shift+Alt+arrows`, `Ctrl+U` | Multi-cursor family (module 06) |
| K10 | `Ctrl+Z`/`Ctrl+Shift+Z`, `Ctrl+L` | History, select-line |

## Execution push channel (`clientExecuteEditorCommand`)

init.js:

```js
import { clientExecuteEditorCommand } from "clay:editor";
clientExecuteEditorCommand({ commandId: "editor.clientSetSelection.selectLine" });
```

| # | Action | Expected |
|---|--------|----------|
| K11 | Cold start with the call above | NOT delivered — no client subscribed yet (expected; advisory) |
| K12 | Open a file, trigger runtime reload via settings appearance switch while connected | init.js reruns; the line under the caret becomes selected — proves op → gate → broadcast → connection → widget dispatch |
| K13 | Change `commandId` to `"application.quit"`, reload | Op rejects ("not a known editor command"); nothing published |
| K14 | Third-party package without `editor-control` permission calls the op | Denied (covered by automated tests; not reachable from init.js by design) |

## Tab command bindings (Phase 22.4)

Tab chords ship as `Global`-scope defaults (module 14, T25–T40); this
section covers the configuration side. Policies: numbering follows the card
order; next/prev wrap; moves never wrap; numbered families are 1-based and
capped at 9 (IDs beyond 9 do not exist). Deep reference:
`docs/reference/clay-js-api/shell/client-tab-*.md` + `examples/init.js`
section 7 (tab annotation block).

init.js:

```js
import { bindKey, unbindKey } from "clay:keybindings";
bindKey("Ctrl+Alt+T", "shell.clientTabNew", { scope: "global" });
```

| # | Action | Expected |
|---|--------|----------|
| K15 | With the init.js above, reload; press `Ctrl+Alt+T` with 2 tabs open | New-tab flow starts (same as `Ctrl+T` / `+`); the shipped default `Ctrl+T` still works — user bindings ADD to defaults |
| K16 | Override a default chord: `bindKey("Ctrl+Tab", "shell.clientTabPrev", { scope: "global" })`, reload, press `Ctrl+Tab` | The override wins — `Ctrl+Tab` now goes to the PREVIOUS tab (user binding beats the shipped default on the same chord); then `unbindKey("Ctrl+Tab", { scope: "global" })`, reload → the default next-tab behavior returns |
| K17 | `bindKey("Ctrl+Alt+9", "shell.clientTabActivate.10", { scope: "global" })` | REJECTED deny-by-default — numbered variants exist only for 1..=9; the diagnostic names the ID |
| K18 | `bindKey("Alt+1", "shell.clientTabActivate.1", { scope: "global" })`; reload; press `Alt+1` with 2 tabs open | Accepted — numbered family IDs bind like any other command ID and activate the first tab; `Alt+2` (unbound) does nothing |

Tab command policy table (module 14 steps in parentheses):

| Command family | Default chord(s) | Policy |
|---|---|---|
| `clientTabNext` / `clientTabPrev` | `Ctrl+Tab` / `Ctrl+Shift+Tab` | wrap around (T25–T26); fewer than 2 tabs = no-op (T28) |
| `clientTabNew` | `Ctrl+T` | same flow as `+`; ignored while the picker is open (T29) |
| `clientTabClose` | `Ctrl+Shift+W` | last tab protected (T31); dirty tabs get the save-all/discard/cancel confirm menu (T32–T35) |
| `clientTabActivate.<N>` | `Ctrl+Alt+<N>` | 1-based card order; N in 1..=9; beyond count = no-op (T27, T39) |
| `clientTabMoveLeft` / `clientTabMoveRight` | `Ctrl+Shift+[` / `]` | boundary = no-op; never wraps (T36–T37) |
| `clientTabMoveTo.<N>` | `Ctrl+Shift+<N>` | 1-based; N in 1..=9; beyond count = no-op (T38) |

## Negative checks

- Key routing never runs package JavaScript in the keypress path.
- Unknown command IDs at runtime map to a no-op result, never a crash.
- No key or intent while a menu session is active reaches the editor: the menu
  route consumes arrows/Enter/Escape/printable/Backspace before editor
  dispatch (keys leak only for unhandled keys; e2e asserts the menu path).

## Plan 087 UI foundation steps

| # | Action | Expected |
|---|--------|----------|
| K69 | Fixture manifest installs `completion.trigger` on `Ctrl+Space` (editor scope, `UiReactivePriority`) | `BehaviorManifestInstalled` log entry contains the binding; pressing `Ctrl+Space` in an editor opens the completion popup; no Rust-side default chord was added |
| K70 | Open the palette (default `Ctrl+X Ctrl+O` chord or fixture binding) with 60+ catalogue entries | Composer-anchored sheet still opens at the composer's width with the retained scrollable list and `{n} results` status; rows stay selectable; Escape closes (plan 124 supersedes the Phase 24.4/24.5 centered geometry — see K92–K99) |
| K71 | Type a filter query in the Command Centre | Results filter live; selected index resets to 0; `{n} results` updates; no editor text leaks |
| K72 | Negative: with a completion or Command Centre session active, attempt editor chords | Menu route consumes arrows/Enter/Escape/printable/Backspace before editor dispatch; keys do not reach the editor |

## Linux execution record (Plan 086 task 11, 2026-08-14)

- **PASS — K19–K24/K29/K47 representative path:** the real Wayland/AT-SPI review instance opened Control Center with the isolated fixture binding, filtered `60` results to `7`, and closed with Escape. The menu/status tree updated through stable virtual IDs (status `14987979559889054209`, items in the following slots); no editor text leaked from menu input and responsiveness was immediate. Evidence: `code-reviews/screenshots/2026-08-14-plan086-a11y/control-center-menu.png`, `control-center-filtered.png`, and `review-log.md`.
- **PASS — manifest/isolation:** the manual instance's `BehaviorManifestInstalled` contained the default `Ctrl+X Ctrl+P` / `Ctrl+X Ctrl+F` sequence bindings plus the isolated fixture overrides. No package or raw-op surface was added; menu labels/status did not expose host paths.
- **Coverage note:** native path/file dialog selection and a second-client observer keyboard flow were blocked by this host's window-targeting backend. Automated chord/deny-by-default coverage remains green.

## Linux execution record (Plan 087 task 11, 2026-08-15)

- **PASS — K69:** the live X11 instance's `BehaviorManifestInstalled` contained `completion.trigger` on `Ctrl+Space` (editor scope, `UiReactivePriority`) and the default `controlCenter.open` `Ctrl+X Ctrl+P` / `controlCenter.openPath` `Ctrl+X Ctrl+F` sequence bindings; `Ctrl+Space` opened the completion popup (module 04 E16).
- **PASS — K70/K71 (non-regression):** the Command Centre open/filter/Escape round trip with 60+ entries was verified live earlier in this plan (task 7): centered `Control Center` Menu `640x206`, 66 results incl. package items, filter `split` → 8 results, Escape close, status `{n} results`; artifacts under `code-reviews/screenshots/2026-08-14-plan087-ui-foundation/command-centre/` (same build).
- **Host limitation (not a false pass):** this session's xdg-desktop-portal keyboard delivery could not hold Ctrl across the two strokes of the `Ctrl+X Ctrl+P` sequence (each combo arrives press+release; the pending-chord timeout is ~1.5 s), so the Command Centre was not re-opened in this instance; the task-7 live capture above plus automated chord tests cover the surface.

## Known ceilings

- Function keys (PageUp/PageDown/F-keys) remain unsupported by the chord
  grammar; the parser accepts single characters, Tab, arrows, Space, Enter,
  Backspace, Delete, and Escape.
- The pending-chord timeout is a server-owned constant (~1.5 s), not user
  configuration.
- Textobject/smart-select IDs ship with NO defaults; binding is the package
  or user's job.

## Plan 089 task 9 Linux execution record (2026-08-17)

| Checks | Result | Evidence |
|---|---|---|
| K73 | PASS | Welcome state captures expose named Open File/Open Folder buttons, polite status, and Connected/Editable status |
| K74 | PASS live | `code-reviews/screenshots/2026-08-14-plan089-platform-validation/visual-review/completion/` shows the bounded completion popup with 44 children, `as` selected, no rows exceeding the visible surface; P1-087-UI-1 containment is visually verified |
| K75 | UNRESOLVED live / PASS structural | Command Centre remains UNRESOLVED because `Ctrl+Alt+P` is consumed by GNOME before reaching Clay; structural clipping/single-scrim/modal-role tests pass |
| K76 | PASS automated | `PackageModalDismiss` routing and Escape intent tests pass; native/package modal keyboard action could not be focused on this host |
| K77 | PASS automated | Shell allowlist, stale-session, package-authority, and key-routing tests pass; Plan 089 added `compact_generated_frame_mutations_fail_closed_without_panicking`, `editor_generated_chord_sequences_preserve_prefix_mismatch_and_timeout_transitions`, and `generated_menu_intent_ordering_preserves_lifecycle_and_authority` |

## Control Center menu round trip (Phase 24.1)

Server-owned interactive menu session (query/selection/activate/cancel
round trip; server-pushed bounded snapshots; client renders and forwards
keystrokes only). Since Phase 24.2, `controlCenter.open` ships with the
default Global-scope `Ctrl+X Ctrl+P` chord (Phase 24.5 sequence default;
pre-24.5 it was `Ctrl+Shift+P`) in the default behavior manifest
and is fully runtime-bindable (allowlist + `bindKey`/`unbindKey`), so the
interactive steps below are runnable by hand on the real Linux build. Each
step also names the automated connection-level e2e that drives the same
wire path a keypress would.

Deep reference: `docs/reference/primitives/shell-layout-strategy.md`
(transient menu family), `docs/reference/packages/creating-packages.md`
(Menu session ownership), `plans/081`, `plans/082`.

| # | Action | Expected |
|---|--------|----------|
| K19 | Open the palette (`Ctrl+X Ctrl+O`, or `controlCenter.open` via `CommandIntent`; automated: `control_center_opens_filters_activates_and_cancels`) | Bottom-anchored sheet opens with prompt “Commands”; bounded catalogue of executable commands (built-in server commands + `shell.client*` + registered package commands; only client-first edit commands excluded); items show label + detail; exactly one snapshot pushed |
| K20 | Type `reload` (automated: `MenuQueryUpdate`) | Server-side filter narrows to `runtime.reloadConfiguration`; query echoed in the snapshot; visuals update only from the pushed snapshot (no optimistic echo) |
| K21 | `ArrowDown` / `ArrowUp` (automated: `MenuSelectionMove ±1`) | Selection moves relative, wraps at list ends; local copy never mutates |
| K22 | `Enter` (automated: `MenuActivate`) | Selected command executes (e2e: `runtime.reloadConfiguration` → `Accepted`); session closes with explicit `TransientMenuClosed` |
| K23 | `Escape` (automated: `MenuCancel`) | Menu closes; no command runs |
| K24 | Type/arrow/Enter while the menu is active | Keys do NOT leak into the editor — document text and caret untouched; menu route consumes them (`dispatch_server_menu_key`) |
| K25 | Open while already open (automated: replacement in the e2e) | Old session replaced: `TransientMenuClosed(old)` then snapshot(new); stale intents for the old id → bounded `menu.unknown_session` diagnostic; connection keeps serving |
| K26 | Switch tabs while the menu is open (automated: `tab_switch_cancels_the_active_server_menu_session`) | Menu dismissed (explicit `TransientMenuClosed`); intents for the old session → `menu.unknown_session`; the other tab's content is unaffected |
| K27 | A local menu opens while the Control Center is open (completion, tab-close confirm; automated: `local_menu_open_cancels_the_active_server_session`) | Client enqueues `MenuCancel` first; exactly one menu renders (one-active-per-tab invariant in both directions) |
| K28 | Package attempts to open/drive the server menu (no init.js/package API exists; security boundary) | Denied by construction — no package facade or SDUI action reaches the intent channel; intents are connection-scoped transport frames; menu activation routes only through registered command authority (see `creating-packages.md` Menu session ownership) |

## Control Center command execution mode (Phase 24.2)

Live generation-safe command catalogue (built-ins + `shell.client*` +
trusted/third-party package registrations merged and stamped with the
runtime generation id), shared bounded fuzzy subsequence matcher, typed
activation dispatch (server commands through the live-registry
`CommandExecutor` boundary, `shell.client*` through the server-approved
`ShellClientCommandRequest` bridge), and the shipped default `Ctrl+X Ctrl+P`
binding. Deep reference: `docs/reference/primitives/registry.md`
(CommandExecution / TransientMenuSession rows), `plans/082`, module 02
(reload) + module 14 (tabs) for the underlying behaviors.

| # | Action | Expected |
|---|--------|----------|
| K29 | Fresh profile (no init.js), press `Ctrl+X Ctrl+O` | Sheet opens with prompt "Commands" — the default Global ServerFirst chord routes through the inert behavior manifest, no hard-coded key in widgets (automated: `client_routes_control_center_open_default_binding_as_server_intent`, `control_center_opens_filters_activates_and_cancels`) |
| K30 | Empty query: inspect the full listing | All 22 built-in server commands (`controlCenter.open`, `controlCenter.openPath`, `runtime.reloadConfiguration`, `workspace.openFuzzyFile`, `language.*`, …) and all 38 `shell.client*` pane/tab entries present (automated: `live_command_catalogue_contains_builtins_and_exact_shell_surface`); with markdown/javascript/typescript/settings packages loaded, their command IDs appear with detail `chord - server-first - @pkg@0.1.0` and built-ins show `built-in` provenance (automated: `command_catalogue_merges_loaded_packages_with_exact_provenance`, `control_center_lists_and_activates_loaded_package_commands`) |
| K31 | Type `ccop` (or `controlcenopen`) | Subsequence fuzzy match ranks "Open Control Center" first even though no substring matches; word-boundary and consecutive matches outrank scattered ones; empty-query order is deterministic by label then id (automated: `src/shell/fuzzy.rs` unit tests) |
| K32 | Type a nonsense query (e.g. `zzzz`) | Empty item list; menu stays open; `Escape` closes without side effects (automated: `catalogue_snapshot_is_not_rebuilt_for_query_updates` empty-items assertion) |
| K33 | Query `splitPaneVertical`, `Enter` (with 1 pane) | Menu closes with explicit `TransientMenuClosed`; a `ShellClientCommandRequest` goes to the client, which re-parses and splits the pane through the same driver path as `Ctrl+\` (automated: `control_center_shell_activation_sends_shell_command_request`) |
| K34 | Query `tabNew` or `tabActivate.2`, `Enter` (2 tabs open) | Same bridge: new tab opens / tab 2 activates via the client shell driver, including dirty-close safety and last-tab protection where applicable |
| K35 | Query `reload`, `Enter` | `runtime.reloadConfiguration` executes: real reload fanout (`runtime.reload_succeeded` diagnostic + `RuntimeStateSnapshot`); the menu was already closed; reopening uses a fresh session id and behavior version |
| K36 | Open a `.md` file, query `togglePreview`, `Enter` | Menu closes; the package JS side effect runs in the markdown package runtime (server-side activation is validation-only — no further wire frame) |
| K37 | Query `settings.open` / `settings.setTheme`, `Enter` | Settings command path runs (right-slot settings panel opens; set* variants persist via `persist_settings_change` then reload) |
| K38 | `Escape` while open | Menu closes; no command runs |
| K39 | Open the menu, switch tabs | Menu dismissed with explicit `TransientMenuClosed`; stale intents for the old session → bounded `menu.unknown_session` diagnostic; the other tab is unaffected (automated: `tab_switch_cancels_the_active_server_menu_session`) |
| K40 | Rebind: in init.js `unbindKey("Ctrl+X Ctrl+O", { scope: "global" })` + `bindKey("Alt+X", "controlCenter.open", { scope: "global" })`, reload | `Alt+X` opens the palette; `Ctrl+X Ctrl+O` does nothing; unbinding `Alt+X` restores the default. `Ctrl+X Ctrl+P` (`shell.toggleAgentLane`) is untouched by this rebind. Overlay semantics: unbind removes only the default chord, bind adds without touching it (automated: `configuration_default_control_center_binding_is_present_and_overridable`, `configuration_default_agent_lane_binding_is_present_and_overridable`, mode-persistence assertion in `control_center_lists_and_activates_loaded_package_commands`) |
| K41 | Open the menu, then trigger a runtime reload (settings appearance switch or `Ctrl+Shift+R`) | Generation replacement cancels the open session (`TransientMenuClosed`) before replaying `RuntimeStateSnapshot`; reopening gives a new session id; stale-session intents → bounded diagnostic (automated: `runtime_generation_replacement_cancels_open_control_center`, `stale_generation_cannot_activate_a_catalogue_item`) |
| K42 | Negative: type/arrow/Enter while the menu is active (fuzzy queries included) | Keys never leak into the editor — text and caret untouched; menu route consumes them (see K24; e2e asserts the menu path) |
| K43 | Negative: stale session — after a tab switch or reopen, send intents with the old session id (select/activate/cancel) | Bounded `menu.unknown_session` Info diagnostic; never an error or disconnect; connection keeps serving (automated: `menu_intents_for_unknown_sessions_produce_bounded_diagnostics`) |
| K44 | Negative: forged shell IDs — a malformed/hostile `ShellClientCommandRequest` (unknown id such as `shell.clientClosePane.evil`, or a raw id outside the 38-entry allowlist) | Client re-parses deny-by-default and drops the request with no state mutation, no crash (automated: `ShellClientCommand::from_command_id` parser tests + client event-mapping test + codec round trip) |
| K45 | Negative: unloaded package commands | On a profile without a package (or after disable), its command IDs are absent from the catalogue; executing a not-listed/unknown id via intent → `UnknownCommand`; the listing grants no execution authority by itself |
| K46 | Security: package UI cannot open/drive menu sessions (see K28) and listing grants no shell/package authority — a listed package command id used from package JS via `serverExecuteCommand` is still re-validated (built-ins → `UnauthorizedTarget`, e.g. reload; permissions/provenance re-checked per activation) | Denied by construction; reserved core IDs (`controlCenter.*`, `shell.*`) cannot be registered by packages; stale-generation package entries cannot activate (automated: `register_command` validation tests, `stale_generation_cannot_activate_a_catalogue_item`) |
| K47 | Performance (qualitative, real Linux build): open the palette with `Ctrl+X Ctrl+O`, type several queries back-to-back | Snapshot-push responsiveness feels immediate; bounded ceilings by design: catalogue ≤ 256 items, one registry snapshot per open, one bounded fuzzy scan + one snapshot per query, no package JavaScript on the query path, no registry rebuild per keystroke (automated bounded-work assertions: `catalogue_snapshot_is_not_rebuilt_for_query_updates`; advisory budgets in module 11 Q11) |

## Path Browser keybinding surface (Phase 24.3)

Built-in server-first browse workflow `controlCenter.openPath` with the
default Global `Ctrl+X Ctrl+F` chord (Phase 24.5 sequence default; the
pre-24.5 temporary default was `Ctrl+Alt+P`, the command id never changed).
File/workspace-level steps live in module 03 (F17–F29); this section covers
the keybinding surface. Deep reference:
`docs/reference/clay-js-api/keybindings/bind-key.md` (Phase 24.3 note),
`docs/reference/clay-js-api/configuration.md` (Phase 24.3 review).

| # | Action | Expected |
|---|--------|----------|
| K48 | Fresh profile (no init.js), press `Ctrl+X Ctrl+F` | Overlay opens, prompt `Browse · <dir>` — the shipped default Global ServerFirst chord routes through the inert behavior manifest like `Ctrl+X Ctrl+O` (automated: `default_keymaps_contain_path_browser_open_binding`, `path_browser_opens_from_keybinding_and_control_center_catalogue`) |
| K49 | Open the palette (`Ctrl+X Ctrl+O`), type `browse` | `controlCenter.openPath` (`Browse Filesystem`) appears in the merged catalogue and opens the path browser on `Enter` — both centre commands are catalogue entries (automated: `path_browser_opens_from_keybinding_and_control_center_catalogue`) |
| K50 | Rebind: init.js `unbindKey("Ctrl+X Ctrl+F", { scope: "global" })` + `bindKey("Alt+P", "controlCenter.openPath", { scope: "global" })`, reload | `Alt+P` opens the path browser; `Ctrl+X Ctrl+F` does nothing; unbinding `Alt+P` restores the default; the command id never changes (automated: `configuration_default_path_browser_binding_is_present_and_overridable`, which asserts the default is present before unbind and the override manifests) |
| K51 | `Escape`, `Enter`, `Alt+Enter`, `Backspace`, arrows, and typing while the path browser is open | Shared menu route (K24/K42 semantics): keys never leak into the editor; Enter/Tab = primary activation, `Alt+Enter` = secondary (directory as tab workspace, module 03 F24), every other Alt-key combo falls through to the editor |
| K52 | Negative: forged/derived IDs — `bindKey("Ctrl+X Ctrl+F", "controlCenter.openPathExtra", …)` or a package registering `controlCenter.openPath` | Rejected — the runtime-bindable allowlist contains exactly the shipped id (automated: task-7 keybinding allowlist tests rejecting sibling/forged ids; reserved core IDs cannot be registered by packages, K46) |
| K53 | Security: package code tries to open or drive the path session (no API exists) | Denied by construction — no package facade or SDUI action reaches the intent channel; `serverExecuteCommand("controlCenter.openPath")` from package JS yields nothing on the wire (automated: `package_command_lane_cannot_open_path_browser`) |

## Centered modal surface and accessibility (Phase 24.4)

**Plan 124 supersession (2026-09-17).** The command and path sessions are no
longer centered: `controlCenter.open` / `controlCenter.openPath` now render the
composer-anchored `/` palette (bottom sheet at the composer's width, one veil
over panes + inspector rail, the lane itself never veiled). K54–K59 below keep
their *semantics* (one surface, one scrim, key/input containment, scrim click
and Escape cancel, bounded work) but their centered geometry, 640-px width
clamp, and prompt name are historical: read them against K92–K99, and the
automated coverage in `frontend/src/command-centre/CommandPalette.test.tsx` +
`frontend/src/shell/WorkspacePanes.test.tsx`. Agent-picker and package dialogs
keep the centered overlay family.

**Plan 125 supersession (2026-09-18).** The centered projection itself is gone,
not just re-anchored: every picker flow (agent type, provider, credentials,
model, session) is now a **stage of the same `/` palette session** with a typed
presentation mode, the secret stage draws its own shielded field, and the
package-UI anchor vocabulary maps the retired `centered` value to a bottom /
working-area projection for wire compatibility. K54–K59's "centered panel"
placement, K70's fixture binding, and K75's centered-era comparison are
historical geometry; their containment/one-surface/one-scrim semantics live on
in K97 and the new K100–K108. Agent-picker and package dialogs no longer render
through the retired origin.

Deep references: `docs/development/accessibility.md`,
`docs/wiki/modules/transient-menu-session.md`,
`docs/wiki/modules/react-sdui-package-ui.md`.

| # | Action | Expected |
|---|--------|----------|
| K54 | Open command mode or path mode | One Spotlight-style centered panel appears over one full-window translucent scrim; splits/tab bar remain visible and dimmed; no duplicate panel appears per pane. |
| K55 | Inspect accessibility tree while centered menu is open | One named modal `Dialog` (`Control Center` or sanitized `Browse · …`) contains one `Menu`, bounded `MenuItem` rows, and one polite live `Status` announcing `0 results`, `1 result`, or `{n} results`. |
| K56 | Type/filter results, then move selection with arrows | Query snapshots update the same retained panel; count changes update same Status node; arrow-only selection with unchanged count does not repeat the count label. |
| K57 | Press Tab/Shift+Tab, arbitrary function/modifier key, paste, or IME input while centered menu is open | Input remains contained: supported menu intent keys work; unsupported keys/paste/IME are consumed and do not alter editor text, caret, or keybinding state. |
| K58 | Click scrim outside panel, then close with Escape | Scrim click changes no document/caret/selection and does not move focus into overlay; Escape closes through server cancel and focus remains on originating pane. |
| K59 | Performance (qualitative, real Linux build): open, type a filter, descend, ascend, jump, cancel | Responsiveness feels immediate; one bounded depth-1 scan per directory change, zero filesystem work per filter keystroke, one snapshot per accepted transition, snapshot under the 1 MiB frame ceiling (automated: `path_browser_navigation_only_creates_no_grants`, `path_browser_snapshot_stays_under_frame_ceiling`; module 03 known ceilings) |

## Plan 124 steps (persistent agent lane + composer palette, 2026-09-17)

Deep references: `DESIGN.md` §6/§7/§12, `plans/124-Persistent-Agent-Lane-and-Slash-Command-Palette.md`,
`design-artifacts/approved/agent-lane-palette/`, `docs/wiki/modules/transient-menu-session.md`.

| # | Action | Expected |
|---|--------|----------|
| K92 | Press `Ctrl+X Ctrl+P` (or click the status bar's `hide lane`/`lane` hint) twice | The tab's persistent agent lane hides and returns; the hint label flips `hide lane` ⇄ `lane` with the chip `Ctrl X P`; the hidden lane leaves the accessibility tree (no `Agent lane` node) while its composer draft survives; no editor text, caret, or pane changes; a second tab keeps its own lane state (K144-family per-tab rule). Automated: `frontend/src/shell/shell-chords.test.tsx`, `frontend/src/shell/WorkspacePanes.test.tsx` (per-tab visibility), `shell.toggleAgentLane` ClientUiCommand tests |
| K93 | Open the palette with `Ctrl+X Ctrl+O`, the titlebar `Control Center` trigger, or the status bar's `palette` hint; repeat on a tab whose lane is hidden | The palette opens as the composer-anchored sheet with prompt `Commands`, and the composer draft is overwritten with `/` (the trigger focuses the composer; an existing draft is replaced). On a tab with the lane hidden, the trigger makes the lane visible first, then opens the sheet. The retired centered `Control Center` sheet does not appear. Automated: `frontend/src/command-centre/CommandPalette.test.tsx`, `frontend/src/coding-agent/Composer.test.tsx`, `client_routes_control_center_open_default_binding_as_server_intent` |
| K94 | Type a free-text query while the palette is open, then use the scope segment (`All` · `Session` · `Shell` · `Files`) | Each query/scope change sends one `menuQueryUpdate` and pushes one snapshot: the count in the foot (`{n} results`) and the rows update live, the selected index resets to the first row, and the active chip is marked. `Files` routes to `controlCenter.openPath`; `Session` shows the daemon's slash commands (measured 14 with the canonical config); `Shell` shows built-in + client-UI commands; `All` shows the merged catalogue. No catalogue rebuild per keystroke. Automated: `CommandPalette.test.tsx` (scope visibility/activation/reset), `Composer.test.tsx` (chip → query/scope wire payload), `WorkspacePanes.test.tsx` (scope on the wire), `catalogue_snapshot_is_not_rebuilt_for_query_updates` |
| K95 | Move the selection with `ArrowUp`/`ArrowDown`, then run a row with `Enter` | Selection moves without re-announcing an unchanged count; `Enter` dispatches the selected command and clears the composer draft. A built-in row runs its command (e.g. `Toggle Agent Lane`, `Open Control Center`, `Browse Filesystem`); a `Shell` row with a chord chip runs that chord's command; a `Session` row (`/compact`, `/new`, `/resume`) goes to the agent; a `Files` row opens the Path Browser as the same bottom-anchored sheet. `Tab` is not a completion key (it passes through to the composer). Automated: `CommandPalette.test.tsx` (activation), `command_centre_lists_and_activates_loaded_package_commands` |
| K96 | Cancel: press `Escape`; then reopen and press `Escape` again without changing the draft; then open the palette and hide the lane with `Ctrl+X Ctrl+P`; then open it and switch tabs | `Escape` closes the sheet and keeps focus in the composer; a second `Escape` without a draft change does not reopen it (the palette stays closed until the draft text changes). Hiding the lane while the sheet is open closes the sheet **and** its veil (no orphan scrim over the panes/rail — the plan-124 launch-test defect D6), and showing the lane restores the sheet with its draft and scope. A tab switch cancels the session (one active server session per tab). Automated: `Composer.test.tsx` (Escape gate), `CommandPalette.test.tsx` (cancel), `WorkspacePanes.test.tsx` (veil closes with the lane; session cancel on switch) |
| K97 | With the palette open, inspect the sheet, the veil, and the accessibility tree | One sheet anchored 6px above the composer box at exactly the composer's width (18px insets, no separate centering token), max-height `min(52vh, 420px)`, spring-snappy rise on open; one `modal.scrim` veil (dim + blur) covering the main panes **and** the inspector rail, and never the titlebar, status bar, or lane (measured live: pane/rail brightness ratio 0.88 each, lane 1.00); one named dialog (`Commands`) with the bounded listbox and a polite `<output>` result count; no per-pane duplicate surface; under `prefers-reduced-transparency` the veil is opaque with no blur. Automated: `frontend/src/test/workspace-composition.test.tsx` (grid placement, lane z=41 above veil z=40), `CommandPalette.test.tsx` (count/output), capture tool `design-artifacts/tools/capture-lane-palette.mjs` |
| K98 | Negative: with the palette open, press editor chords (`Ctrl+D`, `Ctrl+\`, typing), run it against an empty catalogue (fresh profile with no package commands), and open it on an agent-less tab | Menu route consumes its intent keys before editor dispatch — no document/caret mutation and no stray text in the editor; an unmatched query shows the sheet's empty state (`0 results`) with no exception and no fabricated rows; on a tab with no agent the lane keeps its place, the composer stays typable, and the palette still lists `Shell`/client commands. Automated: `CommandPalette.test.tsx` (empty state), `AgentLane.test.tsx` (agent-less composition), menu key-routing tests |
| K99 | Rebind: `unbindKey("Ctrl+X Ctrl+O", { scope: "global" })` + `bindKey("Alt+X", "controlCenter.open", …)` and `bindKey("Alt+L", "shell.toggleAgentLane", …)`, reload | Each chord is independent: `Alt+X` opens the palette, `Alt+L` toggles the lane, and the unbound defaults do nothing; unbinding each restores its default (`Ctrl+X Ctrl+O` / `Ctrl+X Ctrl+P`). `shell.toggleAgentLane` is a client-UI command (server-first, never dropped) and both ids stay in the bindable catalogue. Automated: `configuration_default_control_center_binding_is_present_and_overridable`, `configuration_default_agent_lane_binding_is_present_and_overridable`, `example_config_boot_publishes_the_control_center_chord` |

## Plan 125 steps (palette stage flows + centered-sheet retirement, 2026-09-18)

Deep references: `DESIGN.md` §12 (shell layout / transient surfaces),
`plans/125-Composer-Palette-Stage-Flows-and-Centered-Sheet-Retirement.md`,
`design-artifacts/approved/composer-palette-stages/`,
`docs/reference/ui-components.md`, `docs/reference/packages/creating-packages.md`.

| # | Action | Expected |
|---|--------|----------|
| K100 | Open the palette and activate a picker row (`Choose Agent`, `Choose Provider`, `Choose Model`, `Resume Session`, `Configure Provider`) | The **same** bottom-anchored sheet keeps its width and its 6px gap and swaps to the stage's own prompt (`Agent type`, `Provider`, `Authentication method`, `Model`, `Session`, `Credential`) with the stage's rows; the foot states the stage verb (`choose` · `store` · `save` · `run` · `resume`). No window-centered sheet appears for any origin the wire can carry (the retired `centered` value included). Automated: `WorkspacePanes.test.tsx` (draws a picker stage on the same sheet; draws no centred sheet for any origin), `CommandPalette.test.tsx` (draws a picker stage with its prompt, its filter and its own keys; falls back to a list stage for an unknown mode), `src/server/agent_picker.rs::every_picker_stage_is_a_palette_session_with_its_mode`, `src/server/menu_sessions.rs::no_session_constructor_produces_the_retired_centered_origin` |
| K101 | In a picker stage press `Esc`, then `Alt+Left`; then clear a list filter and press `Backspace` | Each press ascends exactly one stage (credential / URL / OAuth → authentication method → list) while the sheet keeps its rect (no re-anchor, no remount); at the flow entry the session closes on the first press even when the list is filtered (one press per stage) and the draft clears. `Esc` cancels a catalogue/path session instead of ascending. Automated: `Composer.test.tsx` (walks a stage back with Esc and Alt+Left, and cancels only the catalogue), `src/server/agent_picker.rs::stage_back_derives_the_previous_stage_and_drops_the_secret`, `flow_entry_is_the_picker_list_alone`, `leftover_filter_query_does_not_hide_auth_methods`, `src/server/menu_sessions.rs::picker_backspace_walks_the_flow_and_closes_at_its_entry` |
| K102 | Reach the credential stage of a provider flow and type | The sheet draws its own shielded field (`type=password`, bullets on screen, autocomplete/spellcheck off), the **composer field is disabled** and holds no query, and no typed character reaches the composer draft or the persisted layout: the sheet, the server snapshot, and the wire carry the mask only. `Enter` stores through the host credential path; focus returns to the composer when the stage ends or the sheet closes. Automated: `CommandPalette.test.tsx` (shields the credential stage inside the sheet and keeps it out of the DOM; clears the shield when the stage changes or the sheet reopens), `Composer.test.tsx` (types the credential in the sheet; sends the shield), `src/server/agent_picker.rs::secret_is_not_in_snapshot_query_or_labels`, `secret_query_from_masked_snapshot_appends_typed_suffix` |
| K103 | Reach the URL stage (OAuth redirect / custom endpoint) and type a URL | The stage uses the **composer's own field** as its query (no `/` sigil, no second input inside the sheet): `Enter` saves through the stage verb `save`; leaving the stage clears the draft. Automated: `Composer.test.tsx` (runs a row / leaves a stage), `src/server/agent_picker.rs::oauth_stage_offers_browser_open_and_copy_url_actions` |
| K104 | Reach the OAuth device-code stage | The device code and the verification URL arrive as **stage rows** (activatable/copyable) with a poll row; no code is fabricated locally, and cancelling the stage leaves no credential or draft behind. Automated: `src/server/agent_picker.rs::oauth_labels_distinguish_device_code_from_redirect`, `oauth_stage_without_url_keeps_only_the_poll_row` |
| K105 | Move to a session row and press `Alt+Enter`, then `Enter` on the same row | `Alt+Enter` runs the row's declared **secondary** action (`Delete session`, shown as the row's `Alt+↵` chip) and `Enter` resumes it; no picker row other than the session rows declares the secondary chip, and the secondary action never substitutes for the primary one. Automated: `src/server/agent_picker.rs::session_rows_declare_the_delete_binding_and_others_do_not`, `session_primary_resumes_secondary_deletes`, `Composer.test.tsx` (activation, secondary flag on the wire) |
| K106 | Open the catalogue with an existing draft, then open a picker stage; switch catalogue → path → picker → catalogue and close with `Esc` | The `/` sigil is drawn and required only in `catalogue` and `path` modes; a picker stage takes the field verbatim and the stale catalogue query is cleared when the mode changes (no `/query` seeded into a stage). Closing with `Esc` keeps focus in the composer; the palette stays closed until the draft changes. Automated: `Composer.test.tsx` (seeds the sigil and focuses the field when the session opens on its own; closes the palette when the slash goes away), `CommandPalette.test.tsx` (echoes the field as its query and owns no input), `WorkspacePanes.test.tsx` |
| K107 | Negative: with any session open, inspect the shell for a window-centered sheet; then have a package request the retired origin (`TransientMenuOrigin::Centered` / `centered` in a package-UI layout manifest) | Nothing renders centred: 0 centered nodes in the tree, one sheet only, and the retired package-UI anchor value is mapped to a bottom / working-area projection instead of a window-centered rect (wire compatibility only). Automated: `WorkspacePanes.test.tsx` (draws no centred sheet for any origin the wire can carry), `src/server/menu_sessions.rs::no_session_constructor_produces_the_retired_centered_origin`, `src/shell/package_ui.rs` anchor tests, `tests/package_ui_conformance.rs` |
| K108 | Negative/security: from package JavaScript call `serverExecuteCommand("agent.clientOpenAgentPicker")` (and `…ProviderPicker`, `…ModelPicker`, `…ProviderSetup`, `…SessionPicker`, `…SessionSearchPicker`); then try `bindKey("Ctrl+Alt+K", "agent.clientOpenAgentPicker", …)` | The ids resolve as built-in **server-first** commands for the catalogue, but executing them that way opens nothing and produces no rows (no session work), and they are not runtime-bindable, so `bindKey` rejects them: only a user command intent (activating the palette row) opens a picker. Automated: `src/server/command_execution.rs::builtin_picker_commands_stay_inert_command_ids`, `tests/clay_js_doc_registry.rs::plan125_palette_picker_command_ids_are_stable_audited_and_documented`, `plan125_configuration_contract_keeps_picker_flows_palette_owned` |

## Plan 088 interaction and accessibility steps

| # | Action | Expected |
|---|--------|----------|
| K73 | Inspect the welcome shell's Open File/Open Folder actions and status tree | Buttons expose names/actions; the welcome is a Group/panel with a polite Status; connection/access/error state is textual and not color-only |
| K74 | Open completion from the editor, then type, select, dismiss, and attempt a stale accept | Modeless completion keeps editor focus, traps no modal input, consumes only its supported keys, and stale results cannot mutate the document |
| K75 | Open the palette/Path Browser and inspect the sheet + veil (plan 124: bottom-anchored, not centered) | Exactly one token-driven veil and one named Dialog/Menu/Status tree exist; rows are clipped to the sheet, the veil covers panes + rail and never the lane, and focus returns to the composer on Escape |
| K76 | Trigger `PackageModalDismiss` with Escape on a package modal that declares an intent | Escape closes the modal and routes only the declared inert intent; no package JavaScript or native-widget authority runs in the key path |
| K77 | Send unknown/forged shell or package command ids and attempt input while a menu is active | Deny-by-default diagnostics/no-op; no editor text/caret mutation, stale session activation, path leak, or disconnect |
| K84 | Fresh empty-tab welcome: click Open File/Open Folder, then press `Ctrl+X Ctrl+O`, `Ctrl+\\`, and `Ctrl+T` | Native dialog actions emit; palette opens; split and new-tab follow normal routes; no welcome text is inserted |

## Plan 088 task 12 Linux execution record (2026-08-15)

| Checks | Result | Evidence |
|---|---|---|
| K73 | PASS | Current AT-SPI dump in `code-reviews/screenshots/2026-08-15-plan088-task12-manual/default/accessibility.txt` exposes named Open File/Open Folder buttons, Welcome to Clay, polite status, and Connected/Editable status |
| K74 | UNRESOLVED live / PASS structural | Retained Plan 087 completion artifacts and completion unit tests cover modeless selection/dismissal; current Task 8 interactive fixture could not receive targeted keys on this host, so no current live pass is claimed |
| K75 | UNRESOLVED live / PASS structural | Retained Plan 087 Command Centre/Path Browser comparison trees plus clipping, single-scrim, modal-role, and focus-routing tests; current centered interaction was not safely targetable |
| K76 | PASS automated / NOT RUN manually | `PackageModalDismiss` routing and Escape intent tests pass; native/package modal keyboard action could not be focused on this host |
| K77 | PASS automated / NOT RUN manually | Shell allowlist, stale-session, package-authority, and key-routing tests pass; targeted input is blocked by `can_query_windows=false`/`can_focus_windows=false` |
| K84 | PASS automated / NOT RUN manually | `welcome_button_pointer_press_emits_open_file_command` and `welcome_global_keybindings_emit_commands_without_editing_text` exercise real RenderRoot pointer/key dispatch; live desktop input remains host-dependent |

## Plan 097 Phase 9 React Command Centre steps

| # | Action | Expected |
|---|--------|----------|
| K85 | Press `Ctrl+X Ctrl+P` in CodeMirror | One React Aria modal Dialog opens with labelled Search textbox, bounded ListBox/options, selected row, and polite result count; editor text does not change |
| K86 | Type quickly, Backspace, and use ArrowUp/ArrowDown | React sends query/semantic-backspace/relative-selection intents only; displayed query and selection follow server snapshots; selection scrolls inside the bounded list |
| K87 | Press Enter, Alt+Enter, Escape, and click a row | Primary/secondary activation and cancel use the opaque session ID; server closes before dispatch; pointer selection first sends relative movement then activation |
| K88 | Activate pane/tab/editor/client-dialog commands from the catalogue | `ShellClientCommandRequest` passes a closed frontend dispatcher; exact commands reuse workspace/CodeMirror/native-dialog paths; forged sibling IDs do nothing |
| K89 | Activate package, Git, settings, and reload commands | Server registry/provenance/generation validation remains in force; package JS runs server-side only; diagnostics/status update without a parallel frontend executor |
| K90 | Inspect focus with modal open, close with Escape, reopen after tab switch/reload | Focus enters the search field, Tab remains contained, Escape restores origin, and stale/closed sessions cannot act |
| K91 | Empty/error query and narrow window | Empty message and `0 results` are announced; modal remains usable without horizontal overflow at 460 px and uses one scrim/dialog only |

## Plan 097 Phase 9 execution record (2026-08-23)

| Checks | Result | Evidence |
|---|---|---|
| K85–K87/K90 | PASS React interaction/a11y + server suites | `CommandCentre.test.tsx`, menu session/connection tests, and `command-centre-a11y.txt`; React Aria dialog/listbox semantics and Escape restoration are covered. Desktop keyboard backend unavailable, so physical chord replay is not claimed |
| K88–K89 | PASS automated | Workspace closed dispatcher tests, exact-manifest client UI projection test, existing command catalogue/provenance/reload/Git suites |
| K91 | PASS visual + a11y | `command-centre-final.png`, `command-centre-narrow.png`, `command-centre-empty.png` and paired accessibility snapshots under `code-reviews/screenshots/2026-08-23-tauri-react-phase9/` |

## Phase 28 editor-command aliases and package keymaps

Deep references: `docs/reference/packages/creating-packages.md`,
`docs/reference/clay-js-api/keybindings/bind-key.md`, and the command docs
under `docs/reference/clay-js-api/editor/`.

| # | Action | Expected |
|---|---|---|
| K78 | Fresh code profile: press `Ctrl+/`; then repeat in Rust, TypeScript, and JavaScript package modes | The built-in/alias command toggles line comments locally; the package aliases resolve to the same core transform, not a server Accepted no-op |
| K79 | Load `@clay/markdown`; on a plain line press its declared `Ctrl+Shift+8` list chord, then `Ctrl+Alt+1` heading chord | Package `keyRouting` modifier parsing dispatches `markdown.toggleList` and `markdown.insertHeading`; list/ATX transforms are visible and no whole chord is treated as literal character text |
| K80 | Bind `editor.clientToggleFold` and `editor.toggleInlayHints` to editor chords, invoke each, then repeat with a malformed/unknown command such as `markdown.notImplemented` | Known client commands execute through their closed alias/command table; malformed or unbacked IDs fail registration with a diagnostic and never become Accepted no-ops |
| K81 | Bind a two-stroke package-facing transform (`Ctrl+Q Ctrl+W` → `editor.toggleComment`), complete it, mismatch it with printable text, and let it time out | Completion dispatches once; mismatch re-routes the fresh key to the editor; timeout cancels without mutation; package/user chords share the parser and deny-by-default command validation |
| K82 | With a completion, link hover, or command menu active, press editor transform/fold/inlay chords | The active transient/menu route owns its supported keys; no hidden editor mutation or stale command dispatch occurs, and Escape restores pane focus |
| K83 | Load `@clay/markdown`, open a Markdown document, press its declared `Ctrl+Shift+M` preview chord, then press `Ctrl+/` on a commentable line | `Ctrl+Shift+M` routes to `markdown.togglePreview` without inserting text or stealing the editor comment path; `Ctrl+/` still toggles the active Markdown comment prefix. Preview/diagnostic state stays bounded and the client remains connected |

## Phase 28 Linux execution record (2026-08-20)

| Checks | Result | Evidence |
|---|---|---|
| K78, K80–K83 | UNRESOLVED live; PASS structural | The live Entry did not expose editable-text support, so keyboard mutation and preview/comment round trips were not claimed. Closed alias, client-routing, preview registration, malformed-sequence, and menu-consumption tests pass. |
| K79 | PASS automated / NOT RUN live | `parse_keymap_ctrl_shift_m_has_modifiers`, `parse_keymap_multi_stroke_sequence`, and Markdown activation/keymap tests pass; package keymap interaction was not falsely claimed without reliable editor focus. |
| K81 | PASS automated / NOT RUN live | Existing sequence-chord tests cover completion, mismatch, timeout, and no-eaten-typing; the current host could not deliver a reliable two-stroke editor chord. |
| K83 | PASS automated / NOT RUN live | Markdown package tests pin `markdown.togglePreview` on `Ctrl+Shift+M`, while the live preview/comment round trip remains blocked by the same editable-text/input ceiling. |

## Phase 28.7 P2 visual and interaction recapture (2026-08-21)

UI preflight used the UI guidance current at execution time, category `accessibility`, selected
`rams/rams`, and `computer-use-linux_get_app_state` before review. Evidence is
under `code-reviews/screenshots/2026-08-21-phase28.7-p2-recapture/`.

| Checks | Result | Evidence |
|---|---|---|
| K78–K83 | UNRESOLVED live; PASS structural/automated | The host has no development keyboard backend, so package transform, fold/inlay, preview, menu-consumption, and two-stroke live actions were not claimed. Closed alias, keymap, deny-by-default, stale-session, and menu-routing tests pass. |
| Completion / Command Centre trigger | UNRESOLVED live | `completion/review.status` and `command-centre/review.status` record that neither interactive trigger reached Clay; no visual pass was inferred from rest state. |
| Accessibility/focus containment | PASS static/structural; UNRESOLVED interactive | Static shell/recovery trees expose named controls/status/menu roles; keyboard focus return and transient-menu interaction require a keyboard-capable host. |

No existing step was deleted or weakened.

## Plan 097 Phase 12 Tauri/React visual and accessibility review (2026-08-24)

| Check | Result | Evidence |
|---|---|---|
| Command Centre/path modal rest state | PASS static visual/a11y | `command-centre/`, `command-centre-empty/`, and `path-browser/` fixture captures show one labelled modal, Search textbox, bounded options, selected state/count, and no horizontal overflow |
| Query/move/activate/cancel keyboard flow | UNRESOLVED live; PASS component/server tests | No safe development keyboard/window-targeting backend; `CommandCentre.test.tsx`, workspace routing, menu-session, and authority tests pass |
| Modal focus/containment | PASS structural/static | React Aria modal tests and AX snapshots cover dialog semantics; physical focus traversal remains host-blocked |
| Shell status announcement | PASS | `AppShell` footer status now has `role=status` and `aria-live=polite`, locked by `frontend/src/test/shell.test.tsx` |

## Plan 124 execution record (Linux, 2026-09-17)

Live pass on the canonical example config (isolated root, fresh build),
window-cropped captures + AT-SPI; automated legs from the frontend suite (478
tests), the protocol suite (219) and the presentation suite (61).

| Step | Result | Evidence |
|---|---|---|
| K64 (re-aimed: `Ctrl+X Ctrl+P` toggles the lane) | PASS live (route) + automated (matcher) | Live: the status-bar `hide lane`/`lane` hint — which runs the same `agentLane.toggle()` the chord dispatches — hid and restored the lane (0 → 1 `Agent lane` nodes, hint label flipped). Chord matching/prefix-collision: `shell-chords.test.tsx`, `default_keymaps_are_prefix_collision_free`. |
| K70/K84 (palette open, amended row) | PASS live | Live: the titlebar trigger opened the composer-anchored sheet (`Commands`, 95 results, draft `/`); the empty-tab landing's `Open file`/`Open folder` buttons still route natively (module 01 L-series). |
| K92 (lane toggle) | PASS live + automated | See K64 above; the hidden lane leaves the accessibility tree and its draft survives (K96 evidence). |
| K93 (palette open) | PASS live | Live: titlebar trigger and status-bar `palette` hint both opened the sheet and seeded `/`; the retired centered sheet did not appear. |
| K94 (filter + scope) | PASS live (scope) / UNRESOLVED live (typed query) + automated | Live: `Session` chip → `14 results`, chip focused, rows are the daemon slash commands; the free-text path is covered by `CommandPalette.test.tsx`/`Composer.test.tsx`/`WorkspacePanes.test.tsx` (host cannot type into WebKitGTK). |
| K95 (navigate + run) | PASS automated / NOT RUN live | Row activation is not reachable on this host (WebKitGTK exposes palette rows as list items without AT-SPI actions, and no keyboard synthesis); covered by `CommandPalette.test.tsx` + the server menu suites. |
| K96 (cancel/dismissal; defect D6) | PASS live + automated | Live: hiding the lane with the sheet open removed the sheet and the veil (0 dialog nodes), showing the lane restored the draft and the `Session` scope with 14 results. Fixed in `frontend/src/shell/WorkspacePanes.tsx`, pinned in `WorkspacePanes.test.tsx`. `Escape`/tab-switch legs: automated. |
| K97 (anchoring + veil) | PASS live + automated | Live: sheet at the composer's width, one veil over panes + rail (0.88 each), lane/titlebar untouched (1.00); `workspace-composition.test.tsx` pins grid placement and z-order. |
| K98 (negatives) | PASS live (agent-less) + automated | Live: the agent-less lane keeps its place and stays typable. Empty-state, key containment, and unknown-slash cases: `CommandPalette.test.tsx`, `AgentLane.test.tsx`, `Composer.test.tsx`. |
| K99 (rebinding) | PASS automated | `configuration_default_control_center_binding_is_present_and_overridable`, `configuration_default_agent_lane_binding_is_present_and_overridable`, `example_config_boot_publishes_the_control_center_chord` (canonical init.js publishes `Ctrl+X Ctrl+P` → lane and `Ctrl+X Ctrl+O` → palette; live hints/chips render the same pairs). |

Artifacts: `test-plan/artifacts/124-agent-lane/`. Ceilings: no host
keyboard/pointer synthesis (chord strokes, typing, `Enter`, `Escape`) and no
AT-SPI row actions — the automated legs above cover those paths.

## Plan 125 execution record (Linux, 2026-09-18)

Executed on a fresh build with the canonical `examples/config/` tree in an
isolated root; live captures + AT-SPI dumps in `test-plan/artifacts/125-palette/`
(states `01-rest` … `05-lane-restored-palette`, `accessibility.txt`,
`geometry-halo.txt`), plus the fixture layer
(`design-artifacts/tools/capture-lane-palette.mjs`, 291 checks with picker-stage
scenes for providers / auth / secret / URL / OAuth / models / sessions) and the
unit suites named below.

| Step | Result | Evidence |
|---|---|---|
| K100 (picker rows open in the one sheet) | PASS live (row presence + no centred nodes) / PASS automated (stage rendering) | Live: `Configure Provider`, `Choose Agent`, `Choose Model`, `Choose Provider`, `Resume Session` all appear as rows **inside** the one sheet (`dialog Commands 44,531 1124x420`, `95 results`) with 0 centred dialog nodes anywhere in the tree. Stage prompts/modes/verbs are pinned by `every_picker_stage_is_a_palette_session_with_its_mode` and `CommandPalette.test.tsx`; activating a row could not be driven on this host (no input synthesis, rows carry no AT-SPI action) — the fixture tool drives all seven stage scenes. |
| K101 (stage back: Esc / Alt+Left / Backspace) | PASS automated + fixture | `stage_back_derives_the_previous_stage_and_drops_the_secret`, `flow_entry_is_the_picker_list_alone`, `picker_backspace_walks_the_flow_and_closes_at_its_entry`, `leftover_filter_query_does_not_hide_auth_methods`, `Composer.test.tsx` (Esc/Alt+Left ascend, catalogue cancels); fixture scenes step through auth → secret → back. UNRESOLVED live (the stage cannot be reached without row activation). |
| K102 (credential shield) | PASS automated + fixture; PARTIAL live (state inspected, not typed) | Fixture/CDP: the shielded field lives in the sheet, the composer field is `disabled` while the stage is up, and typed characters stay out of the composer draft text; AX exposes the shield's value as the bullet mask only. `CommandPalette.test.tsx` (shields the credential stage inside the sheet and keeps it out of the DOM; clears the shield when the stage changes), `Composer.test.tsx` (types the credential in the sheet / sends the shield), `secret_is_not_in_snapshot_query_or_labels`, `secret_query_from_masked_snapshot_appends_typed_suffix`. Deviation recorded in the plan-125 review: a controlled input mirrors its value in the DOM `value` attribute — standard React behavior, accepted (see the approved-set README). |
| K103 (URL stage uses the composer field) | PASS automated + fixture | `Composer.test.tsx`, `oauth_stage_offers_browser_open_and_copy_url_actions`; fixture captures show the stage prompt `Save` with no second input in the sheet. Documented divergence from the prototype (the URL stage keeps the composer field, per `DESIGN.md` §12). |
| K104 (OAuth device code + poll rows) | PASS automated + fixture | `oauth_labels_distinguish_device_code_from_redirect`, `oauth_stage_without_url_keeps_only_the_poll_row`; fixture scene `stage=oauth`. Deviation recorded: the device code arrives as stage rows rather than the prototype's code block. |
| K105 (`Alt+↵` secondary activation) | PASS automated | `session_rows_declare_the_delete_binding_and_others_do_not`, `session_primary_resumes_secondary_deletes`, `Composer.test.tsx` (secondary flag on the wire). UNRESOLVED live (row activation unavailable). |
| K106 (draft hygiene across mode changes) | PASS live (sigil presence per mode) + automated | Live: the composer draft is exactly `/` for the catalogue sheet and the sheet echoes that field; picker modes are pinned by `Composer.test.tsx` (seeds the sigil only for catalogue/path; clears the draft when the mode changes) and `WorkspacePanes.test.tsx`. Typing a query to observe the transitions is the standing host ceiling. |
| K107 (no centered sheet for any origin) | PASS live + automated | Live: 0 centred nodes in every captured state, one sheet only, and the retired `centered` origin is mapped to a bottom projection (`src/shell/package_ui.rs`); `WorkspacePanes.test.tsx` (draws no centred sheet for any origin the wire can carry), `no_session_constructor_produces_the_retired_centered_origin`, `tests/package_ui_conformance.rs`. |
| K108 (picker command ids inert / unbindable) | PASS automated | `builtin_picker_commands_stay_inert_command_ids` (executing them returns `Accepted` with no session side effect), `plan125_palette_picker_command_ids_are_stable_audited_and_documented` (documented as catalogue rows and action targets, absent from the JS facades and from `is_runtime_bindable_command`), `plan125_configuration_contract_keeps_picker_flows_palette_owned`. |

Ceilings: unchanged for this host — no keyboard/pointer synthesis
(`can_send_development_input: false`), no `EditableText` on the composer over
AT-SPI, and palette rows expose no usable action, so every leg that needs a
typed query, an arrow move, `Enter`, `Alt+Enter`, or a row click is recorded as
UNRESOLVED live and carried by the named automated/fixture legs. The retired
centered-era rows (K54–K59, K70, K75) keep their semantics under the plan-124 +
plan-125 supersession notes; no step was deleted or weakened.

## Plan 129 connection-loop decomposition execution record (2026-09-20)

Regression-only pass over the refactored connection dispatcher. This execution
exercised the Control Center round trip through AT-SPI actions (the shell
exposes a real `palette Ctrl X O` button and the palette rows expose
`DoAction`), on a fresh profile (`run-live.sh start commands`, no init.js).

| Step | Result | Evidence |
|---|---|---|
| K19/K29/K30 (open + catalogue) | PASS live | AT-SPI `press` on the shell's `palette Ctrl X O` button opened the Control Center: `list box "Commands"` with **88 rows** — built-ins (`Reload Configuration and Packages server-first-with-lock — built-in Ctrl+Shift+R`, `Open Control Center server-first — built-in Ctrl+X Ctrl+O`, `Split Pane Vertical client — built-in Ctrl+\`, …), the `shell.client*` family (`Activate Tab 1…9`, `New Tab`, `Close Pane`, `Focus Next Pane`, …), and package commands with provenance/detail (`/compact server-first — @clay/coding-agent@0.1.0`, `Toggle Rust Line Comment server-first — @clay/rust@0.1.0`). One catalogue snapshot pushed for the open. Artifacts: `live-commands/live-commands.txt`, `live-commands-tree2.txt`. |
| K22/K35 (activate a server command) | PASS live | AT-SPI activation of the `Reload Configuration and Packages` row executed `runtime.reloadConfiguration`: the server re-ran `init.js` (`[daemon] agentProfile.register 'coding' applied`, `command.register '/compact' → Err(Rpc("duplicate command"))`), the client stayed connected with no `Session lost`, and the palette kept rendering after the generation replacement. The server-side cancel/replay ordering is pinned by `runtime_generation_replacement_cancels_open_control_center` (fresh run: 28 `control_center` tests passed). Artifacts: `live-commands/live-commands-server-before.log`, `server-after-reload.log`. |
| K21 (MenuSelectionMove) | PARTIAL live | The activated row reported `focused,selected`; relative move/wrap semantics remain covered by the automated menu tests (no arrow keystrokes were deliverable). |
| K33/K34 (client shell bridge) | UNRESOLVED live | AT-SPI activation of `Split Pane Vertical` returned `True` from `DoAction` but produced no second pane/`Pane 2` node, so the AT-SPI row path is not equivalent to `Enter` for client-first commands; the bridge stays pinned by `control_center_shell_activation_sends_shell_command_request`. |
| K20/K23/K24/K31/K38/K42 (query/arrow/Enter/Escape/no-leak) | UNRESOLVED live | Every leg needs typed keys or arrows; this host's `xdg-desktop-portal-gnome` segfaults on RemoteDesktop keyboard sessions, dropping keystrokes (an `Escape` attempt left the palette open with a portal crash logged). Carried by `control_center_opens_filters_activates_and_cancels`, `catalogue_snapshot_is_not_rebuilt_for_query_updates`, `menu_intents_for_unknown_sessions_produce_bounded_diagnostics`, and the shell keep-open assertions. |
| K1–K18, K60–K68 (bindings, chords, sequences) | UNRESOLVED live | Same input ceiling; the sequence-chord fixture (`init-chords.js`) is prepared in the artifact harness for a host that can deliver two-stroke chords. Automated companions unchanged: `route_key_sequence_*`, `configuration_bind_key_prefix_collision_is_rejected`. |
| Automated companions (fresh on the refactored tree) | PASS | `cargo test --lib control_center` 28 passed; `cargo test --lib connection::` 96 passed. |

This record refines the plan-125 ceiling note (“palette rows expose no usable
action”): rows do expose `DoAction` on this host, and server-first activation
provably works, but client-first rows are not driven by it — so the ceiling
stands for the shell-bridge legs rather than for all row activation.
No step was deleted or weakened. Artifacts:
`test-plan/artifacts/129-connection-loop/live-commands/`.
