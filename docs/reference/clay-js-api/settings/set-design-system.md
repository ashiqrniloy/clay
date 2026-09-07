---
id: settings.setDesignSystem
kind: clay-js-api
js_module: clay:settings
js_export: setDesignSystem
js_facade: packages/settings/dist/load.js::setDesignSystem
backing_rust: src/server/command_execution.rs::execute_settings; src/server/connection/runtime.rs::persist_settings_change; src/server/js_runtime/evaluation.rs::apply_persisted_preferences; src/server/ops/theme.rs::apply_design_system
deno_op: op_clay_commands_execute_command
deno_op_path: src/server/ops/commands.rs::op_clay_commands_execute_command
name: setDesignSystem
user_facing_name: Set Design System
summary: Settings-panel command that activates a UI design system (`@clay/core` baseline or a bundled `@clay/design-*` contributor) and persists the choice in `preferences.json` so it survives restarts.
owner: server
phase: Phase 22.4
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: specifier
    type: string
    default: required
    description: "`@clay/core` (built-in baseline) or the name of one bundled package that contributes a `uiDesignSystem` (`@clay/design-neobrutal`, `@clay/design-glass`). Also accepts `item_id` from dropdown action payloads."
security: Validates the specifier server-side and persists an inert preference only: activation applies through the same inert-recipe path as theme.setDesignSystem (recipes reference theme color roles by name and never contain concrete colors, so no raw CSS or color authority exists) and the reload re-validates the stored value through apply_design_system, preserving the previous valid design system on failure. Does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package manager, WASM, client-side JavaScript, raw Deno ops, raw CSS, or renderer callback authority, and never promotes package trust by naming a specifier.
agent_guidance: Use `settings.setDesignSystem` (or the Settings panel "Design system" dropdown) when the user changes the design system interactively; the persisted choice reapplies on every reload. Use `theme.setDesignSystem` from init.js for declarative startup configuration. Never suggest raw CSS or untrusted design-system packages.
lookup_tags: [settings, design-system, preferences, theme, recipes, panel, core, fallback]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setDesignSystem

## Summary

Settings-panel command that activates a UI design system and persists the choice: `@clay/core` selects the built-in baseline, and a bundled `@clay/design-*` package name activates that package's `uiDesignSystem` contribution. The persisted preference reapplies on every runtime reload, so the choice survives restarts.

## Description

`settings.setDesignSystem` is the command-surface settings API behind the Settings panel's "Design system" dropdown. It accepts one string argument (`specifier`, or `item_id` from dropdown action payloads) and executes server-first:

1. The command executor (`src/server/command_execution.rs::execute_settings`) validates the specifier: `@clay/core` (built-in baseline) or a bundled first-party package whose manifest contributes a `uiDesignSystem` (`@clay/design-neobrutal`, `@clay/design-glass`). Anything else fails closed with an `InvalidArguments` diagnostic before any state changes.
2. The connection runtime (`src/server/connection/runtime.rs::persist_settings_change`) persists the validated specifier as the `designSystem` preference in `~/.config/clay/preferences.json` (atomic write) and triggers a runtime generation reload.
3. The reload applies persisted preferences (`src/server/evaluation.rs::apply_persisted_preferences`) through the same activation path as [`theme.setDesignSystem`](../theme/set-design-system.md) (`apply_design_system`), which resolves the declaration against the active theme, fills missing component recipes from the `@clay/core` fallback set, and fails closed on invalid or revoked packages.
4. Every client receives the new `RuntimeStateSnapshot` — including `ui_choices` (the server-enumerated theme/design-system/appearance lists) and `active_design_system` — and the React adapter projects the recipes into CSS custom properties before paint. No restart and no component remount is required.

The command itself exposes no JavaScript module facade; settings intents are command-surface APIs. Programmatic (init.js) activation remains [`theme.setDesignSystem`](../theme/set-design-system.md).

## When to use

Invoke from UI action payloads (Settings panel dropdowns, command centre) or through `commands.serverExecuteCommand` with `commandId: "settings.setDesignSystem"` when the user changes the design system interactively. For startup configuration in `~/.config/clay/init.js`, use [`theme.setDesignSystem`](../theme/set-design-system.md) instead; a persisted settings choice overrides the equivalent `init.js` call because preference apply runs after `init.js` evaluation on every reload.

## JavaScript usage

```ts
import { setDesignSystem } from "clay:settings";

setDesignSystem("@clay/design-neobrutal");
// or
setDesignSystem("@clay/core");
```

`setDesignSystem` sends the `settings.setDesignSystem` command intent through the commands facade and returns the server acceptance status. The Settings panel ships the same command with a Design system dropdown whose items enumerate the server-provided `ui_choices.design_systems` list (`@clay/core` "Core baseline", plus each bundled design-system package).

## Example

```json
{ "commandId": "settings.setDesignSystem", "arguments": { "item_id": "@clay/core" } }
{ "commandId": "settings.setDesignSystem", "arguments": { "specifier": "@clay/design-glass" } }
```

## Options

Pass a single `specifier` (or `item_id`) string. `@clay/core` selects the built-in baseline; other accepted values are the bundled design-system packages (`@clay/design-neobrutal`, `@clay/design-glass`). Third-party or unknown specifiers are rejected before persistence; deeper resolution (enabled record + declaration validation) stays enforced fail-closed at apply time.

## Return and async behavior

Asynchronous: resolves with the server's `accepted` command status once the specifier is validated; persistence and runtime reload then apply the activation, and clients observe the result through the next `RuntimeStateSnapshot` (`active_design_system`, advanced `runtime_generation_id`, refreshed `ui_choices`). Rejects with a command diagnostic error when validation fails.

## Errors

Fails closed with an `InvalidArguments` command diagnostic (`settings.setDesignSystem requires an item_id/specifier argument`, or `settings.setDesignSystem requires an enabled uiDesignSystem contribution, got \`…\``) without persisting or reloading when the specifier is missing, empty, or does not name `@clay/core` or a bundled design-system package. If the stored preference later names a revoked or invalid package, activation fails closed on the next reload: the previous valid design system is preserved and a sanitized diagnostic is recorded.

## Permissions and security

The command requires no permissions: it validates a string and persists an inert preference. Activation reuses the inert-recipe authority boundary of [`theme.setDesignSystem`](../theme/set-design-system.md) — recipes reference theme color roles by name, concrete colors are rejected, and no raw CSS is possible. Naming a specifier never promotes package trust; only bundled first-party design-system packages and the built-in baseline are accepted at the settings layer.

## Agent guidance

Prefer the Settings panel "Design system" dropdown or this command for interactive changes; it persists the choice and reapplies across restarts. Use `theme.setDesignSystem` from init.js for declarative startup configuration. Never suggest raw CSS, concrete colors, or design systems from untrusted packages.

## Backing implementation

Command registration and Settings-panel action targets live in `packages/settings/dist/load.js` and `packages/settings/package.json`. The `setDesignSystem` export wraps `serverExecuteCommand` (`clay:commands`) → `op_clay_commands_execute_command`. `execute_settings` (`src/server/command_execution.rs`) validates the specifier; `persist_settings_change` (`src/server/connection/runtime.rs`) persists the `designSystem` preference and reloads the runtime generation; `apply_persisted_preferences` (`src/server/js_runtime/evaluation.rs`) re-applies it through `apply_design_system` (`src/server/ops/theme.rs`) on every reload. `build_runtime_state_snapshot` + `enumerate_ui_choices` (`src/server/mod.rs`) ship `ui_choices` to every client.

## Lookup metadata

Tags: settings, design-system, preferences, theme, recipes, panel, core, fallback.

## Authority

The command persists a validated preference string only. Activation authority is identical to [`theme.setDesignSystem`](../theme/set-design-system.md): inert recipe data validated server-side, color authority stays with the active theme, and missing component recipes resolve from `@clay/core` fallbacks.

## Denied

Authority not granted: no filesystem, network, shell, extension loading, AI mutation, workspace, package manager, WASM, client-side JavaScript, raw Deno ops, raw CSS, or renderer callback authority; no automatic trust promotion by naming a specifier.

## Key bindings

No default key bindings. The command is invoked from Settings panel dropdown action targets and the command centre.

## Custom properties

- `specifier` (string, required): `@clay/core` for the built-in baseline, or one bundled package contributing a `uiDesignSystem` (`@clay/design-neobrutal`, `@clay/design-glass`). Dropdown payloads may supply the same value as `item_id`.

## Snapshot fields

The runtime state snapshot gained `ui_choices` (`UiChoicesSnapshot`): `themes` (enabled `@clay/theme-*` specifiers), `design_systems` (`@clay/core` first, then enabled `uiDesignSystem` contributors, with server-provided display names), and `appearance` (the persisted `light`/`dark`/`system` preference). The Settings panel renders its dropdowns from this list instead of hardcoded options.
