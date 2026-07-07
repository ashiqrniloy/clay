---
id: clay.behavior.buildCodeEditingManifest
kind: clay-js-api
js_module: "clay:behavior"
js_export: buildCodeEditingManifest
js_facade: runtime/js/behavior.ts::buildCodeEditingManifest
backing_rust: src/server/ops/modes.rs::op_clay_modes_register_pattern
deno_op: op_clay_modes_register_pattern
deno_op_path: src/server/ops/modes.rs::op_clay_modes_register_pattern
name: buildCodeEditingManifest
user_facing_name: Build Code Editing Manifest
summary: Build a generic C-family code-editing behavior manifest (editorRules) from language-specific parameters for use with major mode registration.
owner: server
phase: Phase 18.14
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: indentSize
    type: number
    default: required
    description: Number of spaces used for one indentation level.
  - name: lineComment
    type: string
    default: optional
    description: Line-comment token such as // or #; enables comment continuation.
  - name: blockCommentStart
    type: string
    default: optional
    description: Start token for block comments (reserved for future phases).
  - name: blockCommentEnd
    type: string
    default: optional
    description: End token for block comments (reserved for future phases).
  - name: pairs
    type: Array<{ open: string; close: string }>
    default: [{ "(" / ")" }, { "[" / "]" }, { "{" / "}" }, { '"' / '"' }, { "'" / "'" }]
    description: Delimiter pairs for bracket matching and auto-insertion.
  - name: electricOutdentCharacters
    type: string[]
    default: []
    description: Characters that trigger electric indentation; "}" currently produces outdent-one-level.
  - name: autocompleteTriggers
    type: string[]
    default: []
    description: Trigger characters that may start autocomplete, e.g. [".", "::"].
security: Pure helper emitting inert declarative editor rules. Does not produce executable callbacks, client JavaScript, native handles, or raw authority fields, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use this helper when registering a C-family major mode instead of hand-rolling editorRules that may drift from the server validator.
lookup_tags: [js-api, behavior, manifest, editor-rules, phase18.14]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# buildCodeEditingManifest

## Summary

Build a generic C-family code-editing behavior manifest from language-specific parameters.

## Description

`buildCodeEditingManifest` produces an `editorRules` object matching the shape validated by `clay:modes` registration and activation. It covers indentation size, delimiter pairs, line-comment continuation, electric outdent characters, and autocomplete trigger characters.

The helper is intentionally declarative: it emits only inert metadata and never produces executable callbacks, client-side JavaScript, native handles, or raw authority fields.

## When to use

Use this helper inside a language package load entry to build the `editorRules` passed to `clay.modes.serverRegisterModePattern`. It keeps the package's behavior manifest aligned with the server-side validator and reduces hand-rolled rule drift.

## JavaScript usage

```ts
import { buildCodeEditingManifest } from "clay:behavior";
import { serverRegisterModePattern } from "clay:modes";

const manifest = {
  name: "@vendor/lang",
  version: "0.1.0",
  apiPrefix: "lang",
  permissions: ["mode-registration"]
};

serverRegisterModePattern(manifest, {
  modeId: "lang",
  displayName: "Lang",
  extensions: ["lg"],
  editorRules: buildCodeEditingManifest({
    indentSize: 4,
    lineComment: "//",
    electricOutdentCharacters: ["}"],
    autocompleteTriggers: ["."]
  })
});
```

## Example

```ts
import { buildCodeEditingManifest } from "clay:behavior";

const rules = buildCodeEditingManifest({
  indentSize: 2,
  lineComment: "//",
  pairs: [{ open: "(", close: ")" }, { open: "[", close: "]" }, { open: "{", close: "}" }],
  electricOutdentCharacters: ["}"],
  autocompleteTriggers: ["."]
});

// rules.enter.kind === "preserveLeadingWhitespace"
// rules.tabSpaces === 2
// rules.comments === [{ linePrefix: "//", continuePrefix: "// " }]
```

## Options

- `indentSize` (`number`): spaces per indentation level.
- `lineComment` (`string`, optional): line-comment token.
- `blockCommentStart` (`string`, optional): reserved.
- `blockCommentEnd` (`string`, optional): reserved.
- `pairs` (`Array<{ open: string; close: string }>`, optional): delimiter pairs.
- `electricOutdentCharacters` (`string[]`, optional): electric characters.
- `autocompleteTriggers` (`string[]`, optional): autocomplete trigger characters.

## Key bindings

No default key binding.

## Custom properties

See options above.

## Return and async behavior

Returns a synchronous `editorRules` object with:

- `enter`: `{ kind: "preserveLeadingWhitespace" }`
- `pairs`: filtered non-empty pairs
- `comments`: line-comment continuation rules
- `tabSpaces`: indentation size
- `electricCharacters`: electric outdent rules
- `autocompleteTriggers`: trigger objects

## Errors

Throws a TypeScript runtime error if required fields are missing or malformed.

## Permissions and security

No permission required. Pure helper returning inert declarative rules. Does not execute code, read files, access the network, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Prefer `clay.behavior.buildCodeEditingManifest` when authoring a C-family language package. Do not hand-roll `editorRules` objects that duplicate this shape; if a language needs rules outside the helper's scope, extend the helper rather than bypassing it.

## Backing implementation

- Facade: `runtime/js/behavior.ts::buildCodeEditingManifest`
- Embedded runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_BEHAVIOR`
- Consuming op: `src/server/ops/modes.rs::op_clay_modes_register_pattern`

## Lookup metadata

Lookup tags: `js-api`, `behavior`, `manifest`, `editor-rules`, `phase18.14`.
