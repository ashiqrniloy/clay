# Control Center

Phase 18.8 Task 7: built-in command-palette workflow.

## What it is

The Control Center is a server-owned transient menu that lists executable commands, filters them by query, and routes the selected command through the shared `CommandExecutor`. It is not a bespoke command-palette dispatcher; it reuses the generic `TransientMenuSession` state model, the command registry snapshot, and the existing command execution path.

## Source files

- `src/server/control_center.rs`: `ControlCenter` state, command-to-item projection, query filtering, and selected-item execution.
- `src/server/command_execution.rs`: shared `CommandExecutor`, built-in command table, and `CommandExecutionRequest` validation.
- `src/packages/commands.rs`: `CommandRegistry` snapshot used as the Control Center data source.
- `src/shell/transient_menu.rs`: generic `TransientMenuSession` and `TransientMenuItem` state model.
- `src/shell/package_ui.rs`: projects the active session onto a bottom-anchored transient overlay.
- `src/masonry_sdui.rs` / `src/masonry_editor.rs`: render the overlay and route keyboard navigation/activation/cancel.

## How it works

1. **Open**. `ControlCenter::open(registry, session_id)` reads the current `CommandRegistry` snapshot, filters out commands with client-first or native-client-UI routing policies, appends built-in server commands (`controlCenter.open`, `workspace.refresh`, `document.focus_active`, `document.open_recent`, `modes.listActiveModes`, `modes.explainActiveMode`), and produces a sorted item list.
2. **Display**. Each command becomes a `TransientMenuItem` with a display label, detail string (key binding + routing policy + provenance), accessibility label, provenance (built-in or package), and an inert `TransientMenuAction` carrying only the command ID and empty arguments.
3. **Filter**. `ControlCenter::set_query(query)` returns a new `TransientMenuSession` containing only items whose label, ID, detail, or accessibility label matches the query. Filtering is bounded local string matching; no package JavaScript or IPC runs.
4. **Render**. The filtered session is projected through `TransientPackageOverlay::from_menu_session` onto a bottom-anchored transient overlay and painted by Masonry using existing package-overlay primitives.
5. **Activate**. Keyboard navigation (ArrowUp/ArrowDown) updates selection locally. Enter enqueues a server-first command intent for the selected item's command ID. Escape cancels the session. The command intent is normalized into a `CommandExecutionRequest` and validated/executed by `CommandExecutor`.

## Mode discovery (Phase 18.9)

The Control Center also surfaces two built-in mode-discovery commands for diagnostics: `modes.listActiveModes` (lists every open document's active major mode with provenance and classification source) and `modes.explainActiveMode` (explains one document's active mode, why it was selected, and whether a built-in fallback was used). They are registered as built-in server commands through `CommandDeclaration`, surfaced in the Control Center menu, and resolved through `CommandExecutor::execute_discovery` reading installed `ModeRegistry` state. Because they only read already-installed registry state, they grant no execution, document, or workspace authority: they never trigger filesystem scans, package evaluation, network, shell, AI, WASM, raw ops, or client-side JavaScript. The server-side execution path (`ClayRuntimeOpState::execute_command`) routes the discovery command IDs to `execute_discovery` automatically, so the payload resolves from the live `ModeRegistry` snapshot; other commands continue through the standard validation-only execution path. Provenance is reported as `CoreBuiltIn` (`core.text`/`core.code` always-on fallbacks) or `Package`; the classification source is the recorded `ModePatternKind` (exact filename / wildcard filename / extension / MIME / shebang / bounded leading-content probe / universal fallback).

## Security and authority

- Control Center can only execute commands the server exposes as executable: server-first, server-first-with-lock, ui-reactive-priority, and background routing policies.
- Client-first and client-ui commands are excluded because they require client-side edit authority or native widget coordination.
- Each selected command still passes through `CommandExecutor` validation: unknown commands, invalid provenance, undeclared permissions, malformed/oversize arguments, and unauthorized targets are rejected before any side effect.
- Menu items carry only inert command IDs and bounded JSON arguments; no callbacks, native handles, raw ops, or executable package code.

## Invariants

- Command metadata filtering is bounded by the `TransientMenuSession` item/query budgets.
- No package JavaScript, command side effects, or synchronous IPC run in Masonry paint/layout/pointer/key/text handlers.
- The Control Center does not consume fixed-slot geometry; editor region and caret hit-testing remain unchanged while it is open.

## Tests

- `src/server/control_center.rs`: `opening_control_center_lists_all_executable_commands`
- `src/server/control_center.rs`: `control_center_includes_built_in_commands`
- `src/server/control_center.rs`: `filtering_matches_label_id_binding_and_provenance`
- `src/server/control_center.rs`: `selected_command_executes_through_command_executor`
- `src/server/control_center.rs`: `empty_filtered_session_rejects_execution`
- `src/server/control_center.rs`: `client_first_command_is_not_executable_from_control_center`
- `src/server/control_center.rs`: `item_detail_includes_key_binding_and_provenance`

Run with:

```text
cargo test --lib control_center --quiet
```

## Related

- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/transient-menu-session.md`
- `docs/reference/clay-js-api/commands/server-list-commands.md`
- `docs/reference/clay-js-api/commands/server-register-command.md`
