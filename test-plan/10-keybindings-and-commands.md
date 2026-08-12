# 10 — Keybindings and Commands

`bindKey`/`unbindKey` overrides, deny-by-default validation, command routing,
the `editor-control` execution push channel (`clientExecuteEditorCommand`,
protocol v8), and the Global-scope tab command bindings (Phase 22.4). Deep
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
| K5 | `bindKey("Ctrl+Q Ctrl+W", …)` (multi-stroke) | Rejected — single strokes only (known ceiling) |
| K6 | Bind a textobject ID (`editor.clientSelectTextobject.class.around.current`) | Accepted — auto-declared on first bind; works in grammar files |

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
| `clientTabActivate.<N>` | `Ctrl+<N>` | 1-based card order; N in 1..=9; beyond count = no-op (T27, T39) |
| `clientTabMoveLeft` / `clientTabMoveRight` | `Ctrl+Shift+[` / `]` | boundary = no-op; never wraps (T36–T37) |
| `clientTabMoveTo.<N>` | `Ctrl+Shift+<N>` | 1-based; N in 1..=9; beyond count = no-op (T38) |

## Negative checks

- Key routing never runs package JavaScript in the keypress path.
- Unknown command IDs at runtime map to a no-op result, never a crash.
- No key or intent while a menu session is active reaches the editor: the menu
  route consumes arrows/Enter/Escape/printable/Backspace before editor
  dispatch (keys leak only for unhandled keys; e2e asserts the menu path).

## Known ceilings

- Multi-stroke chords unsupported by `bindKey`.
- Textobject/smart-select IDs ship with NO defaults; binding is the package
  or user's job.

## Control Center menu round trip (Phase 24.1)

Server-owned interactive menu session (query/selection/activate/cancel
round trip; server-pushed bounded snapshots; client renders and forwards
keystrokes only). Since Phase 24.2, `controlCenter.open` ships with the
default Global-scope `Ctrl+Shift+P` chord in the default behavior manifest
and is fully runtime-bindable (allowlist + `bindKey`/`unbindKey`), so the
interactive steps below are runnable by hand on the real Linux build. Each
step also names the automated connection-level e2e that drives the same
wire path a keypress would.

Deep reference: `docs/reference/primitives/shell-layout-strategy.md`
(transient menu family), `docs/reference/packages/creating-packages.md`
(Menu session ownership), `plans/081`, `plans/082`.

| # | Action | Expected |
|---|--------|----------|
| K19 | Open the Control Center (`Ctrl+Shift+P`, or `controlCenter.open` via `CommandIntent`; automated: `control_center_opens_filters_activates_and_cancels`) | Overlay opens with prompt “Control Center”; bounded catalogue of executable commands (built-in server commands + `shell.client*` + registered package commands; only client-first edit commands excluded); items show label + detail; exactly one snapshot pushed |
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
`ShellClientCommandRequest` bridge), and the shipped default `Ctrl+Shift+P`
binding. Deep reference: `docs/reference/primitives/registry.md`
(CommandExecution / TransientMenuSession rows), `plans/082`, module 02
(reload) + module 14 (tabs) for the underlying behaviors.

| # | Action | Expected |
|---|--------|----------|
| K29 | Fresh profile (no init.js), press `Ctrl+Shift+P` | Overlay opens with prompt "Control Center" — the default Global ServerFirst chord routes through the inert behavior manifest, no hard-coded key in widgets (automated: `client_routes_control_center_open_default_binding_as_server_intent`, `control_center_opens_filters_activates_and_cancels`) |
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
| K40 | Rebind: in init.js `unbindKey("Ctrl+Shift+P", { scope: "global" })` + `bindKey("Alt+X", "controlCenter.open", { scope: "global" })`, reload | `Alt+X` opens the Control Center; `Ctrl+Shift+P` does nothing; unbinding `Alt+X` restores the default. Overlay semantics: unbind removes only the default chord, bind adds without touching it (automated: `configuration_default_control_center_binding_is_present_and_overridable`, mode-persistence assertion in `control_center_lists_and_activates_loaded_package_commands`) |
| K41 | Open the menu, then trigger a runtime reload (settings appearance switch or `Ctrl+Shift+R`) | Generation replacement cancels the open session (`TransientMenuClosed`) before replaying `RuntimeStateSnapshot`; reopening gives a new session id; stale-session intents → bounded diagnostic (automated: `runtime_generation_replacement_cancels_open_control_center`, `stale_generation_cannot_activate_a_catalogue_item`) |
| K42 | Negative: type/arrow/Enter while the menu is active (fuzzy queries included) | Keys never leak into the editor — text and caret untouched; menu route consumes them (see K24; e2e asserts the menu path) |
| K43 | Negative: stale session — after a tab switch or reopen, send intents with the old session id (select/activate/cancel) | Bounded `menu.unknown_session` Info diagnostic; never an error or disconnect; connection keeps serving (automated: `menu_intents_for_unknown_sessions_produce_bounded_diagnostics`) |
| K44 | Negative: forged shell IDs — a malformed/hostile `ShellClientCommandRequest` (unknown id such as `shell.clientClosePane.evil`, or a raw id outside the 38-entry allowlist) | Client re-parses deny-by-default and drops the request with no state mutation, no crash (automated: `ShellClientCommand::from_command_id` parser tests + client event-mapping test + codec round trip) |
| K45 | Negative: unloaded package commands | On a profile without a package (or after disable), its command IDs are absent from the catalogue; executing a not-listed/unknown id via intent → `UnknownCommand`; the listing grants no execution authority by itself |
| K46 | Security: package UI cannot open/drive menu sessions (see K28) and listing grants no shell/package authority — a listed package command id used from package JS via `serverExecuteCommand` is still re-validated (built-ins → `UnauthorizedTarget`, e.g. reload; permissions/provenance re-checked per activation) | Denied by construction; reserved core IDs (`controlCenter.*`, `shell.*`) cannot be registered by packages; stale-generation package entries cannot activate (automated: `register_command` validation tests, `stale_generation_cannot_activate_a_catalogue_item`) |
| K47 | Performance (qualitative, real Linux build): open `Ctrl+Shift+P`, type several queries back-to-back | Snapshot-push responsiveness feels immediate; bounded ceilings by design: catalogue ≤ 256 items, one registry snapshot per open, one bounded fuzzy scan + one snapshot per query, no package JavaScript on the query path, no registry rebuild per keystroke (automated bounded-work assertions: `catalogue_snapshot_is_not_rebuilt_for_query_updates`) |

## Path Browser keybinding surface (Phase 24.3)

Built-in server-first browse workflow `controlCenter.openPath` with the
temporary default Global `Ctrl+Alt+P` chord (replaced by sequence defaults in
Phase 24.5 without changing the command id). File/workspace-level steps live
in module 03 (F17–F29); this section covers the keybinding surface. Deep
reference: `docs/reference/clay-js-api/keybindings/bind-key.md` (Phase 24.3
note), `docs/reference/clay-js-api/configuration.md` (Phase 24.3 review).

| # | Action | Expected |
|---|--------|----------|
| K48 | Fresh profile (no init.js), press `Ctrl+Alt+P` | Overlay opens, prompt `Browse · <dir>` — the shipped default Global ServerFirst chord routes through the inert behavior manifest like `Ctrl+Shift+P` (automated: `default_keymaps_contain_path_browser_open_binding`, `path_browser_opens_from_keybinding_and_control_center_catalogue`) |
| K49 | Open the Control Center (`Ctrl+Shift+P`), type `browse` | `controlCenter.openPath` (`Browse Filesystem`) appears in the merged catalogue and opens the path browser on `Enter` — both centre commands are catalogue entries (automated: `path_browser_opens_from_keybinding_and_control_center_catalogue`) |
| K50 | Rebind: init.js `unbindKey("Ctrl+Alt+P", { scope: "global" })` + `bindKey("Alt+P", "controlCenter.openPath", { scope: "global" })`, reload | `Alt+P` opens the path browser; `Ctrl+Alt+P` does nothing; unbinding `Alt+P` restores the default; the command id never changes (automated: `configuration_default_path_browser_binding_is_present_and_overridable`, which asserts the default is present before unbind and the override manifests) |
| K51 | `Escape`, `Enter`, `Alt+Enter`, `Backspace`, arrows, and typing while the path browser is open | Shared menu route (K24/K42 semantics): keys never leak into the editor; Enter/Tab = primary activation, `Alt+Enter` = secondary (directory as tab workspace, module 03 F24), every other Alt-key combo falls through to the editor |
| K52 | Negative: forged/derived IDs — `bindKey("Ctrl+Alt+P", "controlCenter.openPathExtra", …)` or a package registering `controlCenter.openPath` | Rejected — the runtime-bindable allowlist contains exactly the shipped id (automated: task-7 keybinding allowlist tests rejecting sibling/forged ids; reserved core IDs cannot be registered by packages, K46) |
| K53 | Security: package code tries to open or drive the path session (no API exists) | Denied by construction — no package facade or SDUI action reaches the intent channel; `serverExecuteCommand("controlCenter.openPath")` from package JS yields nothing on the wire (automated: `package_command_lane_cannot_open_path_browser`) |
| K54 | Performance (qualitative, real Linux build): open, type a filter, descend, ascend, jump, cancel | Responsiveness feels immediate; one bounded depth-1 scan per directory change, zero filesystem work per filter keystroke, one snapshot per accepted transition, snapshot under the 1 MiB frame ceiling (automated: `path_browser_navigation_only_creates_no_grants`, `path_browser_snapshot_stays_under_frame_ceiling`; module 03 known ceilings) |
