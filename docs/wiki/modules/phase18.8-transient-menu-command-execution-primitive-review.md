# Phase 18.8 Transient Menu and Command Execution Primitive Review

## Source

- `plans/036-Phase18.8-Bottom-Pane-Transient-Menu-and-Command-Execution-Foundation.md`
- `roadmap.md`
- `docs/reference/primitives/registry.md`
- `docs/reference/primitives/shell-layout-strategy.md`
- `docs/wiki/modules/command-registry.md`
- `docs/wiki/modules/server-driven-ui.md`
- `docs/wiki/modules/slot-aware-package-ui.md`
- `docs/wiki/modules/masonry-shell.md`
- `docs/wiki/modules/behavior-manifests.md`
- `docs/wiki/modules/embedded-js-runtime.md`
- `docs/wiki/modules/control-center.md`
- `src/packages/commands.rs`
- `src/server/control_center.rs`
- `runtime/js/commands.ts`
- `src/protocol/sdui.rs`
- `src/masonry_sdui.rs`
- `src/shell/package_ui.rs`
- `src/server/ui.rs`
- `src/server/js_runtime.rs`
- `tests/primitives_docs.rs`

## Overview

Phase 18.8 should add a bottom-pane transient menu and command execution foundation by extending existing generic primitives rather than adding a Control Center-specific widget or package-specific Rust branch. The existing codebase already has command metadata registration, inert SDUI/package UI actions, shell slot and transient overlay state, behavior-manifest routing, package input routing metadata, and a persistent server-side JavaScript runtime. The missing generic pieces are a typed `TransientMenuSession` model and a server-owned `CommandExecution` path that every action source can use.

This review completes the primitive-first gate before implementation. It inventories current command, shell, SDUI/action, behavior-manifest, package UI, and runtime primitives; separates local bounded metadata filtering from server-first command execution; records generic gaps; and states the authority boundary that Phase 18.8 must preserve.

## Existing Primitive Inventory

### Command metadata and behavior routes

- `src/packages/commands.rs::CommandRegistry` is the current source of truth for package-owned command metadata. It validates package provenance, `command-registration`, package-prefixed command IDs, display names, routing policies, key bindings, custom properties, duplicate IDs, undeclared permissions, and executable text-transform fields.
- `runtime/js/commands.ts` exposes `serverRegisterCommand` and `serverListCommands` through the controlled server runtime. These facades serialize package manifests and declarations to Clay-owned ops; raw `Deno.core.ops` names are not public API.
- `docs/reference/clay-js-api/commands/server-register-command.md` and `server-list-commands.md` document registration/listing as runtime-backed metadata APIs. Registration does not grant execution authority.
- `src/protocol/mod.rs`, `src/behavior/manifest.rs`, `src/client/behavior.rs`, and `src/editor/surface.rs` define behavior-manifest command declarations and routing policies. `ClientFirstPredictable` and `ClientFirstRequiresAck` remain Rust-known client edit authorities; package commands must route through server-first/UI/background policies.

### SDUI/action primitives

- `src/protocol/sdui.rs::SduiActionIntent` represents button/list activations as inert command IDs, sources, and bounded primitive arguments.
- `src/server/sdui.rs` validates inbound SDUI actions against commands declared by the active tree, and runtime SDUI publication in `src/server/ops/sdui.rs` validates action targets against built-in and package command registries before publication.
- `src/client/mod.rs` and `src/masonry_sdui.rs` enqueue `ClientMessage::SduiAction` from pointer hits without executing package JavaScript or waiting synchronously for server acknowledgement.
- Existing SDUI action routing is an intent emission boundary, not yet a single command execution service.

### Shell, slot, and transient overlay primitives

- `src/shell/layout.rs` implements internal `WorkingAreaLayout`, `PaneSplitTree`, and `PaneSlotLayout` state. The `bottom` slot already exists as a Clay-owned attachment point.
- `src/masonry_shell.rs` owns the native shell root and places the editor child from installed layout state. Masonry layout reads validated state only; it must not parse packages, run JavaScript, wait on IPC, or mutate package UI state during layout.
- `src/shell/package_ui.rs::PackageUiRuntimeState` stores fixed panels and transient overlays. Accepted overlays render separately from fixed slots and do not consume `PaneSlotLayout` geometry.
- `src/masonry_sdui.rs::SduiNativeState` paints package fixed panels and transient overlays from inert runtime state and can structurally observe panels/overlays without document text, widget handles, raw action authority, raw CSS, or executable code.
- Existing `TransientOverlayContribution` can describe dismissible/focus-scoped overlay intent, but it does not model query text, selected index, filtered item lists, session IDs, activation lifecycle, or typed menu result semantics.

### Package UI, input, state, and configuration primitives

- `src/server/ui.rs::PackageUiRegistry` validates `PanelContribution`, `ComponentContribution`, `TransientOverlayContribution`, `PackageInputContribution`, `PackageUiStateScope`, `PackageLayoutOverride`, and theme-token declarations for package prefix/provenance, supported fields, bounded payloads, action targets, and prohibited authority fields.
- `runtime/js/ui.ts` exposes package UI facades through `clay:ui`; public docs live under `docs/reference/clay-js-api/ui/`.
- `PackageInputContribution` already records inert component/panel/overlay action-routing metadata. It should feed command intents, not introduce raw native event callbacks or client-side JavaScript.
- `PackageUiStateScope` includes `transient-overlay` state scope, but it is schema/lifecycle metadata, not an active command-palette state machine.

### Persistent server runtime primitives

- `src/server/js_runtime.rs::ClayJsRuntimeService` owns the persistent server-side `deno_core` runtime worker for curated `clay:*` facades.
- Runtime evaluation is startup, configuration, package-load, open-time, parse, or explicit command/UI work. It is not called from Masonry paint/layout, pointer, scroll, keypress, text-event, or ordinary local edit application.
- The module loader is deny-by-default and raw platform authorities remain unavailable unless a documented Clay facade grants a constrained subset.
- Phase 18.8 command execution may call server runtime/package handlers only from the server-first execution path, never from client hot paths.

## Generic Phase 18.8 Primitive Gaps

### `CommandExecution`

`CommandExecution` is the missing server-owned path that turns a validated command ID plus bounded arguments and target context into a typed result or diagnostic.

Required shape:

```rust
CommandExecutionRequest {
    command_id,
    arguments,
    target,
    provenance,
    expected_permissions,
}
```

Implementation implications:

- Reuse `CommandRegistry` command metadata instead of creating a second command registry.
- Reject unknown command IDs, stale package provenance, unsupported routing policies, malformed or oversize arguments, undeclared permissions, unauthorized document/workspace targets, and package commands that claim client-first authority.
- Accept intents from SDUI actions, package UI actions, behavior-manifest keybindings, and transient-menu selections through the same request type.
- Keep raw op names internal. A public `clay:commands.serverExecuteCommand` facade should be added only if Phase 18.8 chooses a public programmatic surface and supplies full Clay JS API docs, inventory, generated registry, facade/op, and tests.
- Built-in commands such as `clay.controlCenter.open` may be first-party `clay.*` IDs; package commands must remain package-prefixed.

### `TransientMenuSession`

`TransientMenuSession` is the missing generic typed state model for command palettes and future completion/file/Git pickers. It should use existing shell/overlay/component rendering where possible, but it should not be encoded as a one-off Control Center widget.

Required shape:

```rust
TransientMenuSession {
    session_id,
    prompt,
    query,
    items,
    selected_index,
    actions,
    status,
    focus_policy,
}
```

Implementation implications:

- Store query text, selected index, bounded item list, status text, item provenance, item accessibility label, and inert activation/cancel actions.
- Render through the bottom-pane/transient overlay path while keeping `PaneSlotLayout.main` geometry stable unless a fixed panel is explicitly installed.
- Support cancel/escape/focus restore and stale session rejection.
- Carry inert command intents only; no callbacks, raw ops, native widget handles, raw CSS, renderer callbacks, client-side JavaScript, or hidden authority fields.
- Remain generic enough for future completion, file search, symbol search, Git picker, diagnostics picker, or package-provided quick-pick workflows.

## Local Filtering vs Server-First Execution

Phase 18.8 should classify work explicitly:

| Work | Classification | Allowed path |
| --- | --- | --- |
| Command metadata listing snapshot | Server query/configuration work | `CommandRegistry::list` / Clay facade; not paint/text hot path |
| Query update over installed menu items | Local bounded UI state work | Filter capped item metadata in `TransientMenuSession` |
| Selection movement | Local bounded UI state work | Change selected index; no package JS or IPC wait |
| Render bottom transient menu | Paint/layout read of installed inert state | Masonry reads `TransientMenuSession`/overlay projection only |
| Cancel/escape/focus restore | Local UI state plus optional server notification | No package JS in key/text hot path |
| Activate selected item | Server-first command execution | Enqueue/dispatch `CommandExecutionRequest`; async/cancellable where needed |
| Package handler side effects | Server runtime/command work | Only after command permission/provenance/routing validation |

Ordinary typing, caret movement, local edit application, scroll, paint, layout, pointer hit testing, keypress dispatch, and text-event handling must not synchronously list commands, execute commands, call package JavaScript, wait on IPC, read files, call shell/network/AI, or serialize full documents.

## Rejected Implementation Shapes

- Do not add `ControlCenterWidget`, `ControlCenterCommandRunner`, `MarkdownCommandPalette`, `PackageCommandPalette`, `MarkdownMenu`, or any `if mode == "markdown"` / `if package == "@clay/markdown"` Rust command/menu branch.
- Do not implement command activation separately for SDUI buttons, package UI actions, keybindings, and transient-menu selections. They must normalize to one `CommandExecution` boundary.
- Do not make `TransientOverlayContribution` alone carry active query, selection, result execution, or handler semantics; use a typed `TransientMenuSession` and render/projection adapter.
- Do not expose Masonry `Widget`, `WidgetId`, `WidgetPod`, native handles, direct Masonry widget constructors, Vello/Parley callbacks, raw op names, or raw CSS as package APIs.
- No bottom transient menu path may run package JavaScript, command handlers, package validation, package parsing, configuration evaluation, JavaScript execution, blocking IPC, full-document serialization, filesystem, network, shell, AI, WASM, package-manager, package installation, package enable/disable, raw-op, or client-side JavaScript work in Masonry paint/layout/pointer/scroll/key/text-event handlers.
- Do not treat a public Clay JS API as implemented by adding only a raw op or inventory row; public APIs require facade, op, docs, registry, tests, security notes, and naming metadata.

## Security and Authority Boundary

The Phase 18.8 review introduces no new filesystem, network, shell, AI, WASM, native-widget, raw-op, client-side JavaScript, package-manager, package-install, or package-enable/disable authority.

Allowed authority remains narrow:

- Package command registration is metadata only until `CommandExecution` validates an activation request.
- UI declarations and menu items carry inert action intents only.
- Command arguments are bounded primitive data and must not smuggle callbacks, raw op names, native handles, executable code, arbitrary filesystem paths, credentials, or hidden authority fields.
- Server-side command execution must validate command ID, routing policy, package provenance, required permissions, target document/workspace context, behavior/version/session freshness, and payload budgets before any side effect.
- Registration or display in Control Center does not prove executability for the current context; denied commands must produce typed diagnostics/status rather than partial side effects.

## Planned Documentation and Test Coverage

- `docs/reference/primitives/registry.md` should add `CommandExecution` and `TransientMenuSession` rows as Phase 18.8 generic primitive gaps.
- `docs/reference/primitives/shell-layout-strategy.md` should record the bottom transient menu contract as a specialization of transient panels/overlays plus command intents.
- `tests/primitives_docs.rs` should require this review page to be linked from `docs/wiki/index.md` and `docs/wiki/modules/primitive-architecture.md`, and should assert it records inventory, generic gaps, hot-path classification, rejected Control Center-specific shapes, and no-new-authority text.

## Invariants and Constraints

- `CommandRegistry` remains the command metadata source of truth.
- `CommandExecution` is server-owned; client hot paths may enqueue intents but do not execute package commands locally.
- `TransientMenuSession` is generic UI/session state, not Control Center-specific state.
- Control Center is the first consumer and should be implemented as data/configuration over `TransientMenuSession` plus `CommandExecution`.
- Package JavaScript remains server-side only and outside client paint/layout/input/text-event handlers.
- Menu/session observability must omit document text, secrets, filesystem paths beyond sanitized labels, native handles, raw action payload authority, raw CSS, raw ops, callbacks, and executable package code.

## Tests

- `tests/primitives_docs.rs::phase18_8_transient_menu_command_execution_review_records_inventory_and_gaps`: verifies wiki/index links and required primitive-review contents.
- `src/shell/transient_menu.rs` unit tests cover prompt/query/item bounds, selection wrapping, activation, cancellation, provenance, accessibility labels, focus policy, cancelled-session activation rejection, inert action-only items, and budget truncation for the `TransientMenuSession` implementation.
- `src/server/control_center.rs` unit tests cover opening from the registry snapshot, built-in command inclusion, query filtering by label/id/detail/provenance, selected command execution through `CommandExecutor`, empty-filter rejection, and exclusion of client-first/native-client-UI commands.
- `tests/command_execution.rs` integration/security tests cover unknown command rejection, client-first/client-ui routing rejection, provenance mismatch, undeclared permission, malformed/oversize arguments, invalid document target, workspace-mutation target requirement, and duplicate command ID rejection.
- `tests/package_primitive_gate.rs` covers client-first and client-ui command routing rejection at registration time, alongside existing duplicate/ambiguous/executable-transform validation.
- `src/editor/surface.rs` unit tests verify that ordinary typing updates local text synchronously while server-first keybindings produce only an intent, preserving the no-block-during-typing invariant.
- Future tests should cover session-to-overlay projection and end-to-end SDUI/package/keybinding/menu sources sharing one command execution path.
- Run focused documentation coverage with:

```text
CARGO_TARGET_DIR=target/pi-verify cargo test --test primitives_docs phase18_8_transient_menu_command_execution_review_records_inventory_and_gaps --quiet
```

## Related

- [Command Registry](command-registry.md)
- [Transient Menu Session](transient-menu-session.md)
- [Control Center](control-center.md)
- [Server-Driven UI Protocol Schema](server-driven-ui.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- [Masonry Shell Runtime](masonry-shell.md)
- [Behavior Manifests](behavior-manifests.md)
- [Embedded JavaScript Runtime](embedded-js-runtime.md)
- [Primitive Architecture](primitive-architecture.md)
- [Shell/Layout Strategy Reference](../../reference/primitives/shell-layout-strategy.md)
