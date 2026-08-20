---
id: editor.rotateHeading
kind: clay-js-api
js_module: "clay:editor"
js_export: rotateHeading
js_facade: runtime/js/editor.js::rotateHeading
backing_rust: src/masonry_pane_document.rs::PaneDocumentView::apply_editor_client_command; src/editor/surface/mod.rs::EditorSurface::command(EditorCommand::RotateHeading)
deno_op: op_clay_keybindings_bind_key
deno_op_path: src/server/ops/keybindings.rs::op_clay_keybindings_bind_key
name: rotateHeading
user_facing_name: Rotate Heading
summary: Return the stable command ID for cycling manifest-declared heading prefixes.
owner: client
phase: Phase 28.2
visibility: public
permissions: []
key_bindings: []
custom_properties: []
security: Bindable client-first editor command ID only; cycles inert heading prefixes locally after explicit user routing and does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use `editor.rotateHeading` through `bindKey`; heading prefixes come from the active behavior manifest. Do not add language-specific Rust branches or raw Deno ops.
lookup_tags: [editor, heading, text-transform, keybindings, js-api]
app_visible: true
help_visible: true
stability: runtime-backed-command
async: false
---

# rotateHeading

## Summary

Returns the stable command ID `editor.rotateHeading` for cycling the active line or selection through manifest-declared heading prefixes.

## Description

The facade is a synchronous command-ID helper. After explicit routing, Clay uses the active mode's `heading_prefixes` list and applies the generic line-prefix transform to lines touched by each caret or selection. Empty configuration is a safe no-op; no heading literals are embedded in the client command path.

## When to use

Use this API to bind a heading-cycle command for a mode that declares heading prefixes. Markdown's package-owned alias `markdown.insertHeading` maps to this same core command.

## JavaScript usage

```ts
import { rotateHeading } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+1", rotateHeading(), { scope: "editor" });
```

The equivalent string form is also valid:

```ts
bindKey("Ctrl+Alt+1", "editor.rotateHeading", { scope: "editor" });
```

## Example

```ts
// ~/.config/clay/init.js
import { rotateHeading } from "clay:editor";
import { bindKey } from "clay:keybindings";

bindKey("Ctrl+Alt+1", rotateHeading(), { scope: "editor" });
```

## Options

No options are accepted. Prefix order and formatting come from the active mode manifest.

## Key bindings

No core default key binding is assigned. A package alias may provide a mode-specific default; `bindKey` can assign any additional editor chord.

## Custom properties

No behavior-changing custom properties are defined.

## Return and async behavior

Returns `"editor.rotateHeading"` synchronously. Calling the helper does not mutate a document, call the server, execute package code, or run client-side JavaScript.

## Errors

The helper has no runtime errors. `bindKey` rejects malformed chords, unsupported scopes, or unknown command IDs. A mode without heading prefixes reports a sanitized no-op diagnostic rather than inserting a fixed heading syntax.

## Permissions and security

No additional permission is required. The routed operation is client-local ordinary editor behavior using inert manifest data. It does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Prefer this generic command over Markdown- or language-specific heading code. Add prefix data to the behavior manifest; do not add callbacks, executable transform fields, or parser/render branches.

## Backing implementation

- JS facade: `runtime/js/editor.js::rotateHeading`
- Deno op used for binding: `src/server/ops/keybindings.rs::op_clay_keybindings_bind_key`
- Command mapping: `src/masonry_editor.rs::EditorClientCommand::from_command_id`
- Local transform: `src/masonry_pane_document.rs::PaneDocumentView::apply_editor_client_command` and `src/editor/surface/mod.rs::EditorSurface::command`

## Lookup metadata

- Stable ID: `editor.rotateHeading`
- User-facing name: Rotate Heading
- Module/export: `clay:editor` / `rotateHeading`
- Default key bindings: none
- Custom properties: none
- Tags: `[editor, heading, text-transform, keybindings, js-api]`
