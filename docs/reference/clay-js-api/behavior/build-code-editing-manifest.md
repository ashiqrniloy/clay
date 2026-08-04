---
id: clay.behavior.buildCodeEditingManifest
kind: clay-js-api
js_module: "clay:behavior"
js_export: buildCodeEditingManifest
js_facade: runtime/js/behavior.js::buildCodeEditingManifest
backing_rust: src/server/ops/modes.rs::op_clay_modes_register_pattern
deno_op: op_clay_modes_register_pattern
deno_op_path: src/server/ops/modes.rs::op_clay_modes_register_pattern
name: buildCodeEditingManifest
user_facing_name: Build Code Editing Manifest
summary: Build a generic code-editing behavior manifest (editorRules) from language-specific parameters for use with major mode registration.
owner: server
phase: Phase 18.18
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
  - name: enter
    type: object
    default: preserveLeadingWhitespace
    description: Generic EnterRule declaration; supports preserveLeadingWhitespace, insertNewlineOnly, continueLineMarkers, or preserveFenceBodyIndent.
  - name: pairs
    type: Array<{ open: string; close: string }>
    default: [{ "(" / ")" }, { "[" / "]" }, { "{" / "}" }, { '"' / '"' }, { "'" / "'" }]
    description: Delimiter pairs for bracket matching and auto-insertion.
  - name: electricOutdentCharacters
    type: string[]
    default: []
    description: Unique single characters that trigger the generic outdent-one-level effect.
  - name: autocompleteTriggers
    type: string[]
    default: []
    description: Up to 32 unique single-character autocomplete triggers, e.g. [".", ":"].
  - name: movement
    type: 'object | undefined'
    default: code-editing defaults
    description: Optional movement policy (Plan 071 task 4/11), validated by the server. Fields — wordSeparators ('code', 'prose', or { custom: string[] }), treatUnderscoreAsWord, camelCaseSubWord, paragraphStyle ('blankLine' | 'blankLineOrWhitespace'), stopAtEolWordEnd, lineMovement ('character' | 'screenLine'), stickyColumn. Absent fields fall back to the code-editing defaults.
  - name: caretStyle
    type: 'object | undefined'
    default: editor default bar
    description: Optional caret appearance override (Plan 071 task 6/11), validated by the server. Fields — shape ('bar' | 'line' | 'block' | 'underline'), widthPx, heightPct, hollow, blink ('solid' | 'blink' | 'phase' | 'smooth'), smoothAnimationMs, stopBlinkOnTyping. Absent means the reduced-motion-safe editor default bar; clientSetCursorStyle overrides it at runtime.
security: Pure helper emitting inert declarative editor rules. Does not produce executable callbacks, client JavaScript, native handles, or raw authority fields, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use this helper when registering a language major mode instead of hand-rolling editorRules that may drift from the server validator.
lookup_tags: [js-api, behavior, manifest, editor-rules, phase18.18]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# buildCodeEditingManifest

## Summary

Build a generic code-editing behavior manifest from language-specific parameters.

## Description

`buildCodeEditingManifest` produces an `editorRules` object matching the shape validated by `clay:modes` registration and activation. It covers generic Enter behavior, indentation size, delimiter pairs, line-comment continuation, electric outdent characters, autocomplete trigger characters, and the optional per-mode `movement` and `caretStyle` settings (Plan 071 tasks 4/6/11).

The helper is intentionally declarative: it emits only inert metadata and never produces executable callbacks, client-side JavaScript, native handles, or raw authority fields.

## When to use

Use this helper inside a language package load entry to build the `editorRules` passed to `clay.modes.serverRegisterModePattern`. It keeps the package's behavior manifest aligned with the server-side validator and reduces hand-rolled rule drift. Prose modes pass `movement: { wordSeparators: "prose", treatUnderscoreAsWord: false, camelCaseSubWord: false }`; ligatures are not configured here — they follow the mode's font-role typography profile via `clay.theme.setTypography`.

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
    enter: { kind: "preserveLeadingWhitespace" },
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
- `enter` (`object`, optional): a generic `EnterRule` declaration (`preserveLeadingWhitespace`, `insertNewlineOnly`, `continueLineMarkers`, or `preserveFenceBodyIndent`).
- `pairs` (`Array<{ open: string; close: string }>`, optional): delimiter pairs.
- `electricOutdentCharacters` (`string[]`, optional): unique single-character electric triggers.
- `autocompleteTriggers` (`string[]`, optional): up to 32 unique single-character autocomplete triggers.

## Key bindings

No default key binding.

## Custom properties

See options above.

## Return and async behavior

Returns a synchronous `editorRules` object with:

- `enter`: the supplied generic rule, or `{ kind: "preserveLeadingWhitespace" }`
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

Prefer `clay.behavior.buildCodeEditingManifest` when authoring a language package. Do not hand-roll `editorRules` objects that duplicate this shape; if a language needs rules outside the helper's scope, extend the helper rather than bypassing it.

## Backing implementation

- Facade: `runtime/js/behavior.js::buildCodeEditingManifest`
- Runtime include table: `src/server/facades.rs`
- Consuming op: `src/server/ops/modes.rs::op_clay_modes_register_pattern`

## Lookup metadata

Lookup tags: `js-api`, `behavior`, `manifest`, `editor-rules`, `phase18.18`.
