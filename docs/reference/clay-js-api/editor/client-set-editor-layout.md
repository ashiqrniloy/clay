---
id: editor.clientSetEditorLayout
kind: clay-js-api
js_module: "clay:editor"
js_export: clientSetEditorLayout
js_facade: runtime/js/editor.js::clientSetEditorLayout
backing_rust: src/editor/surface/mod.rs::EditorSurface::set_editor_layout
deno_op: op_clay_editor_set_editor_layout
deno_op_path: src/server/ops/editor.rs::op_clay_editor_set_editor_layout
name: clientSetEditorLayout
user_facing_name: Set Editor Layout
summary: Set the user-owned document wrap-policy override through the `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 26
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: wrapPolicy
    type: enum
    default: required
    description: Document wrap policy; allowed values are none, viewport, and column. `none` disables wrapping and enables horizontal scrolling; `viewport` wraps to the pane content width; `column` wraps to `columnCap` average character widths. The override beats the per-mode manifest `editorRules.layout.wrap`.
  - name: columnCap
    type: number
    default: "72"
    description: Column cap for `column` (clamped to 16–240). Ignored for `none` and `viewport`.
security: Configuration-only rendering customization; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, or document mutation authority.
agent_guidance: Use `editor.clientSetEditorLayout` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [editorlayout, editor, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientSetEditorLayout

## Summary

Set the user-owned document wrap-policy override through the `clay:editor` Clay JavaScript facade.

## Description

`clientSetEditorLayout` is the public API for **Set Editor Layout**. The `op_clay_editor_set_editor_layout` deno op validates typed arguments (deny-by-default enum, clamped column cap), publishes the override to every connected client editor surface, and returns the validated descriptor. The client applies it through `EditorSurface::set_editor_layout`, which takes precedence over the per-mode manifest `editorRules.layout.wrap` and the `WrapPolicy::from_font_role` default. Packages cannot forge this override: the op is registered in the trusted runtime extension only, so third-party package code cannot resolve it; the `editor-control` trust gate additionally allows trusted-domain user configuration (`~/.config/clay/init.js`) outside any package activation.

Authority: `configuration-driven-client-ui-state`. Runtime path: `configuration-api-to-client-ui`. Wrap policy is layout-affecting geometry; changing it invalidates the layout cache key and repaints, but does not route ordinary keypresses through JavaScript or block paint/input on server work. The override survives configuration reload (the channel and current-value store are shared across runtime generations).

## When to use

Use this API when JavaScript configuration (`init.js`) needs to pin a user-owned wrap policy that beats the per-mode default — for example, forcing `none` (horizontal scroll) for code, or `column` for prose. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability. Per-mode defaults stay in the behavior manifest `editorRules.layout.wrap`; this API is the user override on top.

## JavaScript usage

```ts
import { clientSetEditorLayout } from "clay:editor";

clientSetEditorLayout({ wrapPolicy: "none" });
```

## Example

```ts
// Force a 72-column wrap for prose reading, overriding the mode default.
clientSetEditorLayout({ wrapPolicy: "column", columnCap: 72 });
```

## Options

- `wrapPolicy` (`"none" | "viewport" | "column"`): Document wrap policy; required. `"none"` disables wrapping and enables horizontal scrolling; `"viewport"` wraps to the pane content width; `"column"` wraps to `columnCap` average character widths.
- `columnCap` (`number`): Column cap for `"column"`, clamped to 16–240; default `72`. Ignored for `"none"` and `"viewport"`.

## Key bindings

No default key binding is assigned. `clientSetEditorLayout` is a programmatic, argument-bearing API; per-mode defaults are set through the behavior manifest `editorRules.layout`.

## Custom properties

- `wrapPolicy` (`enum`, required): Document wrap policy; allowed values are `none`, `viewport`, and `column`. `none` disables wrapping and enables horizontal scrolling; `viewport` wraps to the pane content width; `column` wraps to `columnCap` average character widths.
- `columnCap` (`number`, default `72`): Column cap for `column` (clamped to 16–240). Ignored for `none` and `viewport`.

## Return and async behavior

Returns the validated command descriptor (`{ commandId, wrapPolicy, columnCap? }`) synchronously. The facade is synchronous and local; the override reaches the client editor surfaces asynchronously through the editor-layout broadcast lane (and on connection initial sync / lag replay).

## Errors

The op fails (deny-by-default) if `wrapPolicy` is missing/null or not one of `none`/`viewport`/`column`, if `columnCap` is present but not a finite integer, or if the options are not valid JSON. A present-but-out-of-range `columnCap` is clamped, not rejected.

## Permissions and security

No additional permission is required beyond access to the running editor session from the trusted domain.

Configuration-only rendering customization; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, or document mutation authority.

The op is registered in the trusted runtime extension only; third-party package code cannot resolve or invoke it, so the user override is package-unforgeable. Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientSetEditorLayout` when the user asks to set the editor wrap policy through the Clay JS API or `~/.config/clay/init.js` customization. Avoid inventing direct Rust calls, raw op names, document mutation, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientSetEditorLayout`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_set_editor_layout` (`op_clay_editor_set_editor_layout`)
- Backing Rust/current owner: `src/editor/surface/mod.rs::EditorSurface::set_editor_layout`
- Transport: `ServerMessage::EditorLayoutOverride` (protocol v18) → `ClientConnectionEvent::EditorLayoutOverride` → `PaneDocumentView::apply_connection_event`; publisher `ClayOpState::publish_editor_layout_override`, lane `ClayJsRuntimeService::subscribe_editor_layout` / `editor_layout_override`.
- Layout: `EditorSurface::resolved_wrap` / `layout_max_width` (Phase 26.6).

## Lookup metadata

- Stable ID: `editor.clientSetEditorLayout`
- User-facing name: Set Editor Layout
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientSetEditorLayout`
- Default key bindings: none
- Custom properties: `wrapPolicy`, `columnCap`
- Tags: `editorlayout`, `editor`, `js-api`