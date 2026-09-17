# Control Center

Phase 18.8 Task 7: built-in command-palette workflow. Phase 24.1: first
server-owned session kind on the transient-menu round trip. Phase 24.2:
command execution mode — generation-stamped live catalogue, shared fuzzy
matching, typed activation with the client shell bridge, and a default
`Ctrl+X Ctrl+O` sequence binding (Plan 124; pre-Plan-124 `Ctrl+X Ctrl+P`).

## What it is

The Control Center is a server-owned transient menu that lists executable commands, filters them by query, and routes the selected command through the shared execution paths. It is not a bespoke command-palette dispatcher; it reuses the generic `TransientMenuSession` state model, the generation-stamped `CommandCatalogue`, and the existing server command / client shell execution paths.

## Source files

- `frontend/src/command-centre/{CommandPalette,CommandCentre}.tsx`: palette and centered non-palette projections.
- `frontend/src/coding-agent/{Composer,Composer.test.tsx}`: composer query/focus and tests.
- `frontend/src/shell/{AgentLane,WorkspacePanes,workspace-controller}.tsx`: lane host, veil, per-tab menu routing.
- `src/server/control_center.rs`: `ControlCenter` state, command-to-item projection, fuzzy query scoring, persisted selection, and typed activation.
- `src/server/mod.rs`: `RuntimeGenerationStore::command_catalogue_snapshot(active_manifest)` — the four-source generation-stamped catalogue.
- `src/server/menu_sessions.rs`: per-connection server session store (`ServerMenuSessions`) hosting the Control Center as a session kind; `ServerMenuSession::activate` produces typed activations.
- `src/server/command_execution.rs`: shared `CommandExecutor`, the 22-entry built-in command table (incl. `controlCenter.openPath`, 24.3), and `CommandExecutionRequest` validation.
- `src/packages/commands.rs`: `CommandRegistry`, `CommandCatalogue::from_sources`, `snapshot()`, and the later-source-wins `from_snapshots` merge used for dispatch.
- `src/client_commands.rs`: `SHELL_CLIENT_COMMAND_CATALOGUE` (38 entries) and the deny-by-default `ShellClientCommand::from_command_id` parser.
- `src/shell/fuzzy.rs`: the shared bounded fuzzy subsequence scorer used for query ranking.
- `src/shell/transient_menu.rs`: generic `TransientMenuSession` and `TransientMenuItem` state model.
- `src/shell/package_ui.rs`: projects the active session onto a bottom-anchored transient overlay.
- `src/protocol/menu.rs` / `src/protocol/mod.rs`: inert snapshot DTO, menu intent frames, and the `ServerMessage::ShellClientCommandRequest { command_id }` wire variant.
- `src/server/connection/mod.rs` (dispatch arms) with the `menus.rs`/`workspace.rs`/`documents.rs`/`tabs.rs`/`runtime.rs` family modules (Plan 090/105): `controlCenter.open` special case, the four menu-intent handlers (session tracking now in `menus.rs` after the Plan 105 dispatch-arm extraction), catalogue/dispatch wiring, and generation-replacement cancel.
- `src/server/js_runtime/mod.rs`: `command_registry_snapshots()` — the (trusted, third-party) inert metadata harvest from both runtime domains.
- `src/client/mod.rs`: `ClientConnectionEvent::ShellClientCommandRequest` forwarding.
- `src-tauri/src/bridge` + `frontend/src/shell/workspace-{envelope,commands}.ts`: ShellClientCommandRequest envelopes reach the React client, which executes the routed client command (Plan 097 Phase 12 removed the native driver; the request surface is unchanged).
- `frontend/src/app/layout/{app-shell,working-area}.tsx` + shell transient-menu components: render the overlay and route keyboard navigation/activation/cancel (React shell since Plan 097 Phase 4/12).

## Plan 124 client projection

The server-side Control Center catalogue remains the source for the composer's
`/` palette, but the old centered command/path surface is retired. A
`TransientMenuOrigin::CommandPalette` snapshot is selected by `WorkspacePanes`
and rendered by `CommandPalette` inside the agent lane's composer field. The
field owns the query and focus; the palette owns neither.

Protocol v31 carries `group`, `bindings`, and `scope` on each menu row, plus the
scope on `MenuQueryUpdate`. The server classifies rows into the closed `All`,
`Session`, `Shell`, and `Files` vocabulary and filters the session before fuzzy
scoring. `CommandPalette` renders scope chips and per-stroke `ClayKbd` chips
without deriving either from command IDs. `Files` selects Path Browser mode;
`Session` exposes `@clay/coding-agent` commands; `Shell` covers Clay client,
editor, and built-in commands.

The palette sheet is a child of `ClayTextField`'s menu slot, exactly the width
of the composer and 6px above it. `WorkspacePanes` renders the modal veil over
the panes and inspector rail at z-index 40, while the lane remains interactive
at z-index 41. Centered `CommandCentre` rendering is retained for picker and
package-dialog origins only.

## How it works

1. **Open**. The connection locks the document, clones the active behavior manifest, and calls `RuntimeGenerationStore::command_catalogue_snapshot(active_manifest)`, which merges four sources in order: the 22 built-in server commands (`builtin_server_command_ids`, incl. `controlCenter.openPath` since 24.3), the 38 declared `shell.client*` entries (`SHELL_CLIENT_COMMAND_CATALOGUE`), the trusted-domain registry snapshot, and the third-party-domain registry snapshot. The merge is deterministic (sorted by display name then command ID), fails closed on duplicate IDs and on catalogues above `TRANSIENT_MENU_MAX_ITEMS`, swaps in effective keybindings from the active behavior manifest, and is stamped with the runtime generation ID. The store then opens `ControlCenter::open_catalogue(catalogue, session_id)`, which filters out client-first edit commands and projects each remaining command to an inert `TransientMenuItem` (label, detail, accessibility label, provenance, inert action). One snapshot per open — the catalogue is never rebuilt per keystroke.
2. **Display**. Each command becomes a `TransientMenuItem` with a display label,
   one detail line (`routing - provenance`), accessibility label, provenance
   (`BuiltIn` for `package_name == "clay"`, else `Package { name, version }`),
   inert action data, closed-vocabulary `scope`, optional `group`, and bounded
   `bindings`. Keybindings shown come from the active behavior manifest (which
   already folds user `bindKey`/`unbindKey` overlays); registered/default
   metadata is the fallback. Protocol v31 carries the row fields separately so
   the frontend can render chips without parsing detail text.
3. **Filter**. `ControlCenter::session` scores every item against the query with the shared bounded fuzzy subsequence matcher (`src/shell/fuzzy.rs`), then sorts by score descending, label, then ID (source order when the query is empty). Ranking rewards word boundaries, consecutive matches, and earlier positions; queries longer than 256 chars score `None`; deterministic ties keep the list stable. No registry re-consultation and no package JavaScript runs per query.
4. **Render**. `WorkspacePanes` routes `CommandPalette`-origin snapshots to
   the composer's `ComposerPalette`; `CommandPalette` paints the bounded sheet
   in the field's menu slot. The veil is a separate working-area grid item.
   Non-palette origins continue through `CommandCentre`'s centered modal
   projection.
5. **Activate**. `ControlCenter::selected_activation(target)` produces a typed `ServerMenuActivation`: `Command(CommandExecutionRequest)` for server/package commands, or `ShellClientCommand(command_id)` for `ClientUiCommand` items. On `MenuActivate`, the connection cancels the session first (pushing `TransientMenuClosed`), then dispatches: command activations go through the shared `execute_command_intent` dispatcher with a live aggregated registry built by `CommandRegistry::from_snapshots([trusted, third_party])` (later source wins; built-ins are omitted because the executor falls back to the built-in table); shell activations go out as the narrow `ServerMessage::ShellClientCommandRequest { command_id }` frame, which the client re-parses deny-by-default via `ShellClientCommand::from_command_id` and routes through `apply_shell_client_command` (tab commands, dirty-close gate, pane commands included).

## Phase 24.1: Server-owned round trip

The Control Center became the proving session kind for the [Transient Menu
Round Trip](transient-menu-round-trip.md):

- **Open**. `controlCenter.open` is special-cased in the `CommandIntent`
  dispatch arm (after the stale-behavior-version gate): the store
  (`ServerMenuSessions::open_control_center`) replaces any previous session
  and the arm pushes `TransientMenuClosed(replaced)` plus the bounded
  snapshot. The JS `clay.commands.executeCommand` op path still returns
  bare `Accepted` — opening UI requires the transport `CommandIntent` path.
- **Persisted selection**. `ControlCenter` stores `selected_index`;
  `move_selection(delta)` walks `delta.rem_euclid(len)` `select_next` steps,
  and `session()` chains `with_query` + `with_selected_index` so every
  produced session carries the live query and selection. Arrow intents
  (`MenuSelectionMove`) never mutate server state locally.
- **Filter**. `MenuQueryUpdate` → `set_query` (clamped at the store choke
  point); the snapshot echoes the query (no optimistic client echo). An
  unchanged query keeps `selected_index` — the webview flushes the same
  draft via `menuQuery` right before `menuActivate` on Enter, and that
  flush must not clobber the arrow-selected item; only a genuinely
  changed filter resets the selection to 0.
- **Lifecycle**. Tab switch cancels the session (`cancel_active` + explicit
  closed message); reopen replaces; a local menu opening enqueues
  `MenuCancel` from the pane view; disconnect drops the loop-local store.
  Stale ids get the bounded `menu.unknown_session` diagnostic, never a
  disconnect. Picker/path-browser activations swap ATOMICALLY: the
  replacement session is built first (its inventory can take seconds on
  first model discovery), and only then is the old session reported
  closed ahead of the new snapshot — the modal never vanishes-then-
  reappears mid-transition.
  panic or disconnect.

## Phase 24.2: Command execution mode

- **Default bindings**. `controlCenter.open` ships as a built-in server-intent
  command with `Ctrl+X Ctrl+O` (`Global`, `ServerFirst`);
  `shell.toggleAgentLane` is the sibling client-UI command on `Ctrl+X Ctrl+P`,
  and `controlCenter.openPath` uses `Ctrl+X Ctrl+F`. All remain
  runtime-bindable, so `bindKey`/`unbindKey` can rebind or remove them. The
  palette trigger and chord focus the composer and seed `/`; the lane toggle
  preserves its draft when hidden.
- **Live catalogue**. The menu reflects the runtime's current command
  registry: built-ins, the full `shell.client*` surface, and every
  validated package command from both trust domains — loaded packages
  appear without re-evaluation or registration-time hacks.
- **Generation stamping**. `open_control_center(catalogue, generation_id)`
  stamps the session; `ServerMenuSession::activate(current_generation_id)`
  rejects a replaced runtime with `CommandExecutionRule::StaleRuntimeGeneration`
  (bounded diagnostic), and the runtime generation-replacement broadcast
  cancels the open session with `TransientMenuClosed` before replaying the
  new `RuntimeStateSnapshot`. A reopened menu then lists the new catalogue.
- **Typed activation**. Nothing executes inside the session model;
  `selected_activation` returns the typed enum and the connection owns
  response ordering. Package commands execute validation-only server-side
  (the real JS side effect runs in the package runtime via its own op);
  `ClientUiCommand` items bridge to the client shell driver through the
  server-approved `ShellClientCommandRequest` frame.

## Mode discovery (Phase 18.9)

The Control Center also surfaces two built-in mode-discovery commands for diagnostics: `modes.listActiveModes` (lists every open document's active major mode with provenance and classification source) and `modes.explainActiveMode` (explains one document's active mode, why it was selected, and whether a built-in fallback was used). They are registered as built-in server commands through `CommandDeclaration`, surfaced in the Control Center menu, and resolved through `CommandExecutor::execute_discovery` reading installed `ModeRegistry` state. Because they only read already-installed registry state, they grant no execution, document, or workspace authority: they never trigger filesystem scans, package evaluation, network, shell, AI, WASM, raw ops, or client-side JavaScript. The server-side execution path (`ClayRuntimeOpState::execute_command`) routes the discovery command IDs to `execute_discovery` automatically, so the payload resolves from the live `ModeRegistry` snapshot; other commands continue through the standard validation-only execution path. Provenance is reported as `CoreBuiltIn` (`core.text`/`core.code` always-on fallbacks) or `Package`; the classification source is the recorded `ModePatternKind` (exact filename / wildcard filename / extension / MIME / shebang / bounded leading-content probe / universal fallback).

## Security and authority

- The Control Center can only list commands the server exposes as executable: server-first, server-first-with-lock, ui-reactive-priority, background, and client-UI routing policies. Only `ClientFirstPredictable` / `ClientFirstRequiresAck` edit commands stay excluded (they require built-in Rust client edit authority).
- Listing grants no authority: a `shell.client*` item is inert on the wire; activation ships the narrow server-approved `ShellClientCommandRequest { command_id }` frame and the client re-parses it deny-by-default (`ShellClientCommand::from_command_id` — unknown or forged IDs are dropped with no state mutation). Packages cannot emit, request, or influence that frame or any activation path.
- Each selected command still passes through `CommandExecutor` validation: unknown commands, invalid provenance, undeclared permissions, malformed/oversize arguments, and unauthorized targets are rejected before any side effect.
- The catalogue merge trusts only the two runtime trust domains (verified bundled inventory vs third-party), rejects duplicate IDs across domains fail-closed, and cannot be polluted by packages claiming reserved core IDs (`register_command` namespace rules plus the reserved-domain check). Stale generations cannot activate a stamped session.
- Menu items carry only inert command IDs, bounded arguments, scope/group
  metadata, and binding strings; no callbacks, native handles, raw ops, or
  executable package code. `shell.toggleAgentLane` remains client-local
  per-tab layout state, not server/runtime generation state.

## Invariants

- Command metadata filtering is bounded by the `TransientMenuSession` item/query budgets; the full catalogue must fit `TRANSIENT_MENU_MAX_ITEMS` or open fails explicitly.
- Exactly one catalogue snapshot per menu open and one bounded fuzzy scan per query; no registry rebuild and no package JavaScript on query/paint paths.
- No package JavaScript, command side effects, or synchronous IPC run in Masonry paint/layout/pointer/key/text handlers.
- The composer palette does not consume editor fixed-slot geometry; its veil
  occupies only the working-area grid row and the lane occupies the full-width
  shell row below it. Editor-region and caret hit-testing remain unchanged.

## Tests

- `src/server/control_center.rs`: `opening_control_center_lists_all_executable_commands`, `control_center_includes_built_in_commands`, `filtering_matches_label_id_binding_and_provenance`, `selected_command_produces_command_activation`, `selected_shell_client_item_produces_shell_activation`, `empty_filtered_session_rejects_activation`, `client_first_command_is_not_executable_from_control_center`, `shell_client_catalogue_entries_are_visible_and_parser_allowlisted`, `item_detail_includes_key_binding_and_provenance`, `catalogue_snapshot_is_not_rebuilt_for_query_updates`
- `src/server/menu_sessions.rs`: high-bit ids, replace, query filter, selection wrap, typed activation, cancel, projection, `cancel_active`, adversarial ordering, `stale_generation_cannot_activate_a_catalogue_item`
- `src/server/mod.rs`: `src/server/tests.rs::live_command_catalogue_contains_builtins_and_exact_shell_surface`, `command_catalogue_merges_loaded_packages_with_exact_provenance`
- `src/server/connection/tests.rs`: `control_center_opens_filters_activates_and_cancels`, `control_center_shell_activation_sends_shell_command_request`, `control_center_lists_and_activates_loaded_package_commands`, `runtime_generation_replacement_cancels_open_control_center`, `tab_switch_cancels_the_active_server_menu_session`, `menu_intents_for_unknown_sessions_produce_bounded_diagnostics`
- `src/shell/fuzzy.rs`: subsequence vs substring, word-boundary and consecutive bonuses, case-insensitivity, Unicode safety, empty-query and over-long-query behavior
- `src/client/mod.rs`: event mapping for `ShellClientCommandRequest`; `src/client/behavior.rs`: default-binding routing
- `frontend/src/command-centre/{CommandPalette,CommandPalette.test.tsx}`:
  scope/binding fields, rows, empty state, count, and activation
- `frontend/src/shell/{AgentLane,WorkspacePanes,shell-chords}.test.tsx`:
  palette/lane integration and default chord surface
- `src/server/ops/keybindings.rs`: `control_center_open_is_bindable_and_server_routed`

Run with:

```text
cargo test --lib control_center --quiet
cargo test --lib menu_sessions --quiet
cargo test --lib server::connection::tests --quiet
cargo test --lib shell::fuzzy --quiet
```

## Related

- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/transient-menu-session.md`
- `docs/wiki/modules/transient-menu-round-trip.md`
- `docs/wiki/modules/path-browser.md` — Phase 24.3 sibling session kind sharing the open/wiring path
- `docs/wiki/modules/fuzzy-matching.md`
- `docs/reference/clay-js-api/commands/server-list-commands.md`
- `docs/reference/clay-js-api/commands/server-register-command.md`
- `docs/reference/clay-js-api/keybindings/bind-key.md`
- [React Command Centre and Desktop Workflows](react-command-centre-desktop-workflows.md) — current palette/veil projection
- `plans/082-Phase24.2-Command-Execution-Mode.md`
- `plans/124-Persistent-Agent-Lane-and-Slash-Command-Palette.md`
