---
id: clay.completion.completionTriggerCharactersFromEditorRules
kind: clay-js-api
js_module: "clay:completion"
js_export: completionTriggerCharactersFromEditorRules
js_facade: runtime/js/completion.js::completionTriggerCharactersFromEditorRules
backing_rust: src/server/ops/modes.rs::op_clay_modes_register_pattern
deno_op: op_clay_completion_register_completion_provider
deno_op_path: src/server/ops/completion.rs::op_clay_completion_register_completion_provider
name: completionTriggerCharactersFromEditorRules
user_facing_name: Completion Trigger Characters from Editor Rules
summary: Extract trigger characters from behavior-manifest editorRules.autocompleteTriggers so completion provider triggerCharacters stay aligned with mode autocomplete triggers.
owner: server
phase: Phase 18.14
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: editorRules
    type: object
    default: required
    description: Editor rules object with an autocompleteTriggers array of { trigger: string } entries, such as the value returned by clay.behavior.buildCodeEditingManifest.
security: Pure helper; returns inert strings only. Does not execute code, read files, access the network, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.
agent_guidance: Use this helper when registering a completion provider so its triggerCharacters match the mode's autocompleteTriggers.
lookup_tags: [js-api, completion, helper, trigger, phase18.14]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# completionTriggerCharactersFromEditorRules

## Summary

Derive completion provider `triggerCharacters` from a mode's behavior-manifest `editorRules.autocompleteTriggers`.

## Description

`completionTriggerCharactersFromEditorRules` is a pure JavaScript helper that extracts non-empty trigger strings from `editorRules.autocompleteTriggers`. It lets a language package keep a single source of truth for autocomplete triggers in its behavior manifest and reuse the same list when registering a completion provider.

## When to use

Use this helper inside a package load entry after building editor rules (for example with `clay.behavior.buildCodeEditingManifest`) and before calling `clay.completion.serverRegisterCompletionProvider`.

## JavaScript usage

```ts
import { buildCodeEditingManifest } from "clay:behavior";
import {
  completionTriggerCharactersFromEditorRules,
  serverRegisterCompletionProvider
} from "clay:completion";

const editorRules = buildCodeEditingManifest({
  indentSize: 4,
  lineComment: "//",
  autocompleteTriggers: [".", "::"]
});

serverRegisterCompletionProvider({
  packageName: "@vendor/lang",
  packageVersion: "0.1.0",
  packagePrefix: "lang",
  permissions: ["completion-provider"],
  providerId: "lang.keywords",
  triggerCharacters: completionTriggerCharactersFromEditorRules(editorRules)
});
```

## Example

```ts
import { buildCodeEditingManifest } from "clay:behavior";
import { completionTriggerCharactersFromEditorRules } from "clay:completion";

const rules = buildCodeEditingManifest({
  indentSize: 2,
  lineComment: "//",
  autocompleteTriggers: ["."]
});

const triggers = completionTriggerCharactersFromEditorRules(rules);
// triggers === ["."]
```

## Options

- `editorRules` (`object`): Editor rules containing `autocompleteTriggers`, an array of objects with a `trigger` string property.

## Key bindings

No default key binding.

## Custom properties

- `editorRules`: the source editor rules object.

## Return and async behavior

Returns a `string[]` of trigger characters, omitting empty or missing values. The function is synchronous and has no side effects.

## Errors

Throws a standard TypeScript runtime error if `editorRules` is not object-like.

## Permissions and security

No permission required. Pure helper returning inert strings. Does not execute code, read files, access the network, and does not grant filesystem, workspace, network, shell, extension loading, AI mutation, package, WASM, raw Deno ops, native widget, or client-side JavaScript authority.

## Agent guidance

Use `clay.completion.completionTriggerCharactersFromEditorRules` to keep completion provider triggers synchronized with mode behavior manifests. Do not re-implement the extraction inline.

## Backing implementation

- Facade: `runtime/js/completion.js::completionTriggerCharactersFromEditorRules`
- Runtime include table: `src/server/facades.rs`
- Related op: `src/server/ops/completion.rs::op_clay_completion_register_completion_provider`
- Related mode registration: `src/server/ops/modes.rs::op_clay_modes_register_pattern`

## Lookup metadata

Lookup tags: `js-api`, `completion`, `helper`, `trigger`, `phase18.14`.
