# Control Center

Phase 18.8 Task 7: built-in command-palette workflow. Phase 24.1: first
server-owned session kind on the transient-menu round trip. Phase 24.2:
command execution mode — generation-stamped live catalogue, shared fuzzy
matching, typed activation with the client shell bridge, and a default
`Ctrl+X Ctrl+P` sequence binding (Phase 24.5; pre-24.5 `Ctrl+Shift+P`).

## What it is

The Control Center is a server-owned transient menu that lists executable commands, filters them by query, and routes the selected command through the shared execution paths. It is not a bespoke command-palette dispatcher; it reuses the generic `TransientMenuSession` state model, the generation-stamped `CommandCatalogue`, and the existing server command / client shell execution paths.

## Source files

- `src/server/control_center.rs`: `ControlCenter` state, command-to-item projection, fuzzy query scoring, persisted selection, and typed activation.
- `src/server/mod.rs`: `RuntimeGenerationStore::command_catalogue_snapshot(active_manifest)` — the four-source generation-stamped catalogue.
- `src/server/menu_sessions.rs`: per-connection server session store (`ServerMenuSessions`) hosting the Control Center as a session kind; `ServerMenuSession::activate` produces typed activations.
- `src/server/command_execution.rs`: shared `CommandExecutor`, the 22-entry built-in command table (incl. `controlCenter.openPath`, 24.3), and `CommandExecutionRequest` validation.
- `src/packages/commands.rs`: `CommandRegistry`, `CommandCatalogue::from_sources`, `snapshot()`, and the later-source-wins `from_snapshots` merge used for dispatch.
- `src/masonry_shell/window_tabs.rs`: `SHELL_CLIENT_COMMAND_CATALOGUE` (38 entries) and the deny-by-default `ShellClientCommand::from_command_id` parser.
- `src/shell/fuzzy.rs`: the shared bounded fuzzy subsequence scorer used for query ranking.
- `src/shell/transient_menu.rs`: generic `TransientMenuSession` and `TransientMenuItem` state model.
- `src/shell/package_ui.rs`: projects the active session onto a bottom-anchored transient overlay.
- `src/protocol/menu.rs` / `src/protocol/mod.rs`: inert snapshot DTO, menu intent frames, and the `ServerMessage::ShellClientCommandRequest { command_id }` wire variant.
- `src/server/connection/mod.rs`: `controlCenter.open` special case, the four menu-intent handlers, catalogue/dispatch wiring, and generation-replacement cancel.
- `src/server/js_runtime/mod.rs`: `command_registry_snapshots()` — the (trusted, third-party) inert metadata harvest from both runtime domains.
- `src/client/mod.rs`: `ClientConnectionEvent::ShellClientCommandRequest` forwarding.
- `src/app_driver.rs` / `src/masonry_shell/mod.rs`: client re-parse and `apply_shell_client_command` driver routing.
- `src/masonry_sdui.rs` / `src/masonry_pane_document.rs`: render the overlay and route keyboard navigation/activation/cancel.

## How it works

1. **Open**. The connection locks the document, clones the active behavior manifest, and calls `RuntimeGenerationStore::command_catalogue_snapshot(active_manifest)`, which merges four sources in order: the 22 built-in server commands (`builtin_server_command_ids`, incl. `controlCenter.openPath` since 24.3), the 38 declared `shell.client*` entries (`SHELL_CLIENT_COMMAND_CATALOGUE`), the trusted-domain registry snapshot, and the third-party-domain registry snapshot. The merge is deterministic (sorted by display name then command ID), fails closed on duplicate IDs and on catalogues above `TRANSIENT_MENU_MAX_ITEMS`, swaps in effective keybindings from the active behavior manifest, and is stamped with the runtime generation ID. The store then opens `ControlCenter::open_catalogue(catalogue, session_id)`, which filters out client-first edit commands and projects each remaining command to an inert `TransientMenuItem` (label, detail, accessibility label, provenance, inert action). One snapshot per open — the catalogue is never rebuilt per keystroke.
2. **Display**. Each command becomes a `TransientMenuItem` with a display label, detail string (`keybinding - routing - provenance`, e.g. `Ctrl+Shift+M - server-first - @clay/markdown@0.1.0`), accessibility label, provenance (`BuiltIn` for `package_name == "clay"`, else `Package { name, version }`), and an inert `TransientMenuAction` carrying only the command ID and empty arguments. Keybindings shown come from the active behavior manifest (which already folds user `bindKey`/`unbindKey` overlays); registered/default keybinding metadata is the fallback.
3. **Filter**. `ControlCenter::session` scores every item against the query with the shared bounded fuzzy subsequence matcher (`src/shell/fuzzy.rs`), then sorts by score descending, label, then ID (source order when the query is empty). Ranking rewards word boundaries, consecutive matches, and earlier positions; queries longer than 256 chars score `None`; deterministic ties keep the list stable. No registry re-consultation and no package JavaScript runs per query.
4. **Render**. The filtered session is projected through `TransientPackageOverlay::from_menu_session` onto a bottom-anchored transient overlay and painted by Masonry using existing package-overlay primitives.
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
  point); the snapshot echoes the query (no optimistic client echo).
- **Lifecycle**. Tab switch cancels the session (`cancel_active` + explicit
  closed message); reopen replaces; a local menu opening enqueues
  `MenuCancel` from the pane view; disconnect drops the loop-local store.
  Stale ids get the bounded `menu.unknown_session` diagnostic, never a
  panic or disconnect.

## Phase 24.2: Command execution mode

- **Default binding**. `controlCenter.open` ships as a built-in server-intent
  command in the default behavior manifest with a two-stroke `Ctrl+X Ctrl+P`
  sequence rule (Phase 24.5; `KeyBindingRule::global_server_first_sequence`:
  `Global` context, `ServerFirst` routing), shared by
  `minimal_text_editing` and `core_code_editing`. It is
  in the `is_runtime_bindable_command` allowlist, so `bindKey`/`unbindKey`
  configuration overlays can rebind or remove it; the chord survives mode
  activation because every published mode manifest starts from the shared
  default commands/keymaps. Phase 24.3 adds the sibling
  `controlCenter.openPath` with a temporary Global `Ctrl+Alt+P` default in
  the same allowlist, replaced by the `Ctrl+X Ctrl+F` sequence default in
  Phase 24.5 without changing the id — see [Path Browser](path-browser.md)
  and [Sequence Keybindings](sequence-keybindings.md).
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
- Menu items carry only inert command IDs and bounded JSON arguments; no callbacks, native handles, raw ops, or executable package code.

## Invariants

- Command metadata filtering is bounded by the `TransientMenuSession` item/query budgets; the full catalogue must fit `TRANSIENT_MENU_MAX_ITEMS` or open fails explicitly.
- Exactly one catalogue snapshot per menu open and one bounded fuzzy scan per query; no registry rebuild and no package JavaScript on query/paint paths.
- No package JavaScript, command side effects, or synchronous IPC run in Masonry paint/layout/pointer/key/text handlers.
- The Control Center does not consume fixed-slot geometry; editor region and caret hit-testing remain unchanged while it is open.

## Tests

- `src/server/control_center.rs`: `opening_control_center_lists_all_executable_commands`, `control_center_includes_built_in_commands`, `filtering_matches_label_id_binding_and_provenance`, `selected_command_produces_command_activation`, `selected_shell_client_item_produces_shell_activation`, `empty_filtered_session_rejects_activation`, `client_first_command_is_not_executable_from_control_center`, `shell_client_catalogue_entries_are_visible_and_parser_allowlisted`, `item_detail_includes_key_binding_and_provenance`, `catalogue_snapshot_is_not_rebuilt_for_query_updates`
- `src/server/menu_sessions.rs`: high-bit ids, replace, query filter, selection wrap, typed activation, cancel, projection, `cancel_active`, adversarial ordering, `stale_generation_cannot_activate_a_catalogue_item`
- `src/server/mod.rs`: `live_command_catalogue_contains_builtins_and_exact_shell_surface`, `command_catalogue_merges_loaded_packages_with_exact_provenance`
- `src/server/connection/mod.rs`: `control_center_opens_filters_activates_and_cancels`, `control_center_shell_activation_sends_shell_command_request`, `control_center_lists_and_activates_loaded_package_commands`, `runtime_generation_replacement_cancels_open_control_center`, `tab_switch_cancels_the_active_server_menu_session`, `menu_intents_for_unknown_sessions_produce_bounded_diagnostics`
- `src/shell/fuzzy.rs`: subsequence vs substring, word-boundary and consecutive bonuses, case-insensitivity, Unicode safety, empty-query and over-long-query behavior
- `src/client/mod.rs`: event mapping for `ShellClientCommandRequest`; `src/client/behavior.rs`: default-binding routing
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
- `plans/082-Phase24.2-Command-Execution-Mode.md`
