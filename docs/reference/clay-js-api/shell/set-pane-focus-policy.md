---
id: shell.setPaneFocusPolicy
kind: clay-js-api
js_module: "clay:shell"
js_export: setPaneFocusPolicy
js_facade: runtime/js/shell.js::setPaneFocusPolicy
backing_rust: src/server/ops/shell.rs::op_clay_shell_set_pane_focus_policy; src/masonry_shell.rs::ClayShellWidget::set_pane_focus_policy
deno_op: op_clay_shell_set_pane_focus_policy
deno_op_path: src/server/ops/shell.rs::op_clay_shell_set_pane_focus_policy
name: setPaneFocusPolicy
user_facing_name: Set Pane Focus Policy
summary: Set the pane-focus policy (click or cursor) that controls how split panes are activated by the pointer; applied live without restart.
owner: client
phase: Phase 22.1
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: paneFocusPolicy
    type: enum
    default: click
    description: One of `click` (default) or `cursor`. `click` activates a pane on pointer-down inside it. `cursor` activates a pane when the pointer moves over it (focus follows cursor); focus changes are skipped while dragging a divider or panel resize handle.
security: Accepts only the bounded `click` | `cursor` enum and publishes an inert `ShellPreferences` snapshot to the client shell widget; does not grant filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, WASM, raw Deno ops, client-side JavaScript, or document authority.
agent_guidance: Use setPaneFocusPolicy({ paneFocusPolicy: "cursor" }) from init.js to enable focus-follows-cursor across split panes. The default is click-to-focus. Do not suggest values outside the closed click | cursor enum.
lookup_tags: [shell, panes, splits, focus-policy, click, cursor, init]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setPaneFocusPolicy

## Summary

Set the pane-focus policy (`click` | `cursor`) that controls how split panes are activated by the pointer. The default is `click`. The setting applies live (no restart) whenever `~/.config/clay/init.js` is evaluated or reloaded.

## Description

`setPaneFocusPolicy` stores one bounded shell preference and publishes it to every connected client as an inert `ShellPreferences` snapshot. The client shell widget maps the value to its pane activation behavior:

- `click` (default): a pointer-down inside an inactive pane activates it. Editor panes activate through the editor's own pointer-down focus; placeholder panes activate via the shell's click-to-focus handler.
- `cursor`: moving the pointer over a pane activates it without a click. Focus changes are skipped while a divider drag or panel resize drag is in progress, and only when more than one pane exists.

Tab/Shift+Tab pane focus cycling (active when more than one pane exists) is preserved under both policies. The setting is transported server→client through the same broadcast lane used by the caret style override, so late-joining and reconnecting clients replay the current value.

Phase 22.2 keeps this policy as the sole pane-activation configuration surface: file-open flows, duplicate-open focus routing, and the open-documents switcher all target the pane this policy keeps active, and none of them introduce configuration options of their own (fixed product behavior).

Phase 22.3 (tabs as independent client views): the policy is **per active tab** — each tab is a separate connection carrying its own `ShellPreferences` snapshot, and the value governs pane activation inside the active tab's split tree only (inactive tabs keep their own policies and are not pointer-interactive). The configuration surface is unchanged; no tab-related option is introduced.

## When to use

Use from `~/.config/clay/init.js` to switch pane activation between click-to-focus and focus-follows-cursor. Most users keep the default (`click`); `cursor` suits users who work with several panes and prefer tiling-window-manager focus behavior.

## JavaScript usage

```ts
import { setPaneFocusPolicy } from "clay:shell";

setPaneFocusPolicy({ paneFocusPolicy: "cursor" });
```

`setPaneFocusPolicy` returns `{ paneFocusPolicy }` — the stored value.

## Example

```ts
// ~/.config/clay/init.js
import { setPaneFocusPolicy } from "clay:shell";

// Focus follows the pointer across split panes.
setPaneFocusPolicy({ paneFocusPolicy: "cursor" });
```

## Options

Pass `{ paneFocusPolicy }` where `paneFocusPolicy` is a string.

## Return and async behavior

Synchronous. Returns `{ paneFocusPolicy: "click" | "cursor" }`. The helper calls one Deno op; it does not run server IPC, package JavaScript, or client-side JavaScript.

## Errors

Throws `shell.invalid_pane_focus_policy` for:

- input that is not valid JSON or not an object with a `paneFocusPolicy` string;
- unknown values (anything outside the closed `click` | `cursor` enum) with a diagnostic naming the offending value and the two valid options.

## Permissions and security

Authority not granted: no filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget handles, WASM, raw `Deno.core.ops`, client-side JavaScript, or document authority. The input is bounded to the closed `click` | `cursor` enum; the published snapshot is inert data that only changes client-side pointer activation behavior.

## Agent guidance

Use `setPaneFocusPolicy({ paneFocusPolicy: "cursor" })` only when the user explicitly asks for focus-follows-cursor. Leave the default (`click`) otherwise. Do not suggest arbitrary strings or configuration keys outside this API.

## Backing implementation

`runtime/js/shell.js::setPaneFocusPolicy` calls `op_clay_shell_set_pane_focus_policy` (`src/server/ops/shell.rs`), which validates the bounded enum and calls `ClayOpState::publish_shell_preferences`. The value is broadcast to connected clients as `ServerMessage::ShellPreferences` (protocol version 10), delivered as `ClientConnectionEvent::ShellPreferences`, and applied by `ClayShellWidget::set_pane_focus_policy` (`src/masonry_shell.rs::PaneFocusPolicy::from_config_str` maps `"cursor"` → `FollowsCursor`, anything else → `ClickToFocus`).

## Lookup metadata

Tags: shell, panes, splits, focus-policy, click, cursor, init.

## Authority

Only the bounded `click` | `cursor` enum is accepted. The setting grants no authority; it changes only which client-side pointer interaction activates a pane. No theme JavaScript, package parser, or raw IPC runs in paint, layout, scroll, keypress, text-event, or edit-ack hot paths.

## Denied

Authority not granted: no filesystem, network, shell, package manager, extension loading, workspace mutation, clipboard, AI mutation, native widget, client-side JavaScript, WASM authority, or promotion-by-naming. Out-of-enum values are rejected with `shell.invalid_pane_focus_policy`.

## Key bindings

No default key bindings. This API is a startup configuration surface for `init.js`, not a key routing target.

## Custom properties

- `paneFocusPolicy` (enum, default `click`): one of `click` or `cursor`. `click` activates a pane on pointer-down inside it. `cursor` activates a pane when the pointer moves over it (focus follows cursor); focus changes are skipped while dragging a divider or panel resize handle.
