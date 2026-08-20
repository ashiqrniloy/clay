---
id: editor.clientMoveCursor
kind: clay-js-api
js_module: "clay:editor"
js_export: clientMoveCursor
js_facade: runtime/js/editor.js::clientMoveCursor
backing_rust: src/editor/surface/mod.rs::EditorSurface::move_word_start
deno_op: op_clay_editor_move_cursor
deno_op_path: src/server/ops/editor.rs::op_clay_editor_move_cursor
name: clientMoveCursor
user_facing_name: Move Cursor
summary: Move the caret through the `clay:editor` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: [ArrowLeft, ArrowRight, ArrowUp, ArrowDown, Home, End, Ctrl+Home, Ctrl+End, Ctrl+Left, Ctrl+Right, Ctrl+Up, Ctrl+Down]
custom_properties:
  - name: direction
    type: enum
    default: none
    description: Movement direction (nextWordStart, prevWordStart, nextWordEnd, prevWordEnd, nextParagraph, prevParagraph, firstNonWhitespace, lastNonWhitespace, matchingPair, left, right, up, down, start, end).
  - name: granularity
    type: enum
    default: none
    description: Optional motion granularity (word, subword, paragraph, line, character).
  - name: extend
    type: boolean
    default: false
    description: Whether movement extends the current selection.
  - name: count
    type: number
    default: 1
    description: Repeat count for the motion (clamped to >= 1).
security: Changes only client-local caret/selection/viewport state and grants no document mutation or external authority; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `editor.clientMoveCursor` only for its documented editor responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [cursormovement, editor, js-api]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# clientMoveCursor

## Summary

Move the caret through the `clay:editor` Clay JavaScript facade.

## Description

`clientMoveCursor` is the public API for **Move Cursor**. The `op_clay_editor_move_cursor` deno op validates typed arguments (deny-by-default enum) and returns the validated command descriptor. Key-driven movement is served client-local by the direction-specific `editor.clientMoveCursor.*` command IDs (allowlisted, routed `ClientUiCommand`, dispatched in `EditorWidget`).

Authority: `client-local-ui-state`. Runtime path: `client-local-hot-path`. Arrow/Home/End and Ctrl+arrow word/paragraph movement update local caret/viewport state without IPC, server work, or JavaScript.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Move Cursor` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { clientMoveCursor } from "clay:editor";

clientMoveCursor({ direction: "nextWordStart", extend: false, count: 1 });
```

## Example

```ts
clientMoveCursor({ direction: "nextParagraph", extend: true });
```

## Options

- `documentId` (`string`, optional): Target editor/document surface.
- `direction` (`enum`): `nextWordStart` | `prevWordStart` | `nextWordEnd` | `prevWordEnd` | `nextParagraph` | `prevParagraph` | `firstNonWhitespace` | `lastNonWhitespace` | `matchingPair` | `left` | `right` | `up` | `down` | `start` | `end`.
- `granularity` (`enum`, optional): `word` | `subword` | `paragraph` | `line` | `character`.
- `extend` (`boolean`): Whether movement extends the current selection; defaults to `false`.
- `count` (`number`): Repeat count for the motion; defaults to `1` (clamped to >= 1).

## Key bindings

Default key bindings:

- `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`
- `Home`, `End`, `Ctrl+Home`, `Ctrl+End`
- `Ctrl+Left` (previous word start), `Ctrl+Right` (next word start)
- `Ctrl+Up` (previous paragraph), `Ctrl+Down` (next paragraph)
- Add `Shift` to any of the above to extend the selection.

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js` using the direction-specific command IDs (e.g. `editor.clientMoveCursor.nextParagraph`).

## Custom properties

- `direction` (`enum`): Movement direction (see Options).
- `granularity` (`enum`, optional): Motion granularity.
- `extend` (`boolean`, default `false`): Extend the current selection.
- `count` (`number`, default `1`): Repeat count.

## Return and async behavior

Returns the validated command descriptor (`{ commandId, direction, granularity, extend, count }`) synchronously. The facade is synchronous and local.

## Errors

The op fails (deny-by-default) if `direction` is missing or not one of the documented values, if `granularity` is present but unknown, or if the options are not valid JSON.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Changes only client-local caret/selection/viewport state and grants no document mutation or external authority; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `editor.clientMoveCursor` when the user asks to move cursor through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/editor.js::clientMoveCursor`
- Deno op: `src/server/ops/editor.rs::op_clay_editor_move_cursor` (`op_clay_editor_move_cursor`)
- Backing Rust/current owner: `src/editor/surface/mod.rs::EditorSurface::move_word_start` (and `move_paragraph`, `move_first_non_blank`, `move_last_non_blank`, `move_matching_pair`)
- Key-driven dispatch: `src/masonry_editor.rs::EditorWidget::apply_editor_client_command`

## Lookup metadata

- Stable ID: `editor.clientMoveCursor`
- User-facing name: Move Cursor
- Kind: `clay-js-api`
- Module/export: `clay:editor` / `clientMoveCursor`
- Default key bindings: `ArrowLeft`, `ArrowRight`, `ArrowUp`, `ArrowDown`, `Home`, `End`, `Ctrl+Home`, `Ctrl+End`, `Ctrl+Left`, `Ctrl+Right`, `Ctrl+Up`, `Ctrl+Down`
- Custom properties: `direction`, `granularity`, `extend`, `count`
- Tags: `cursormovement`, `editor`, `js-api`
