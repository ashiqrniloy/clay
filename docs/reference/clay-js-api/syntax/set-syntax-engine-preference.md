---
id: clay.syntax.setSyntaxEnginePreference
kind: clay-js-api
js_module: "clay:syntax"
js_export: setSyntaxEnginePreference
js_facade: runtime/js/syntax.js::setSyntaxEnginePreference
backing_rust: src/server/syntax.rs::SyntaxGrammarRegistry::set_engine_preference
deno_op: op_clay_syntax_set_engine_preference
deno_op_path: src/server/ops/syntax.rs::op_clay_syntax_set_engine_preference
name: setSyntaxEnginePreference
user_facing_name: Set Syntax Engine Preference
summary: Force a first-party syntax engine tier for a language/package during init.js or package load.
owner: server
phase: Phase 18.16
visibility: public
permissions: []
key_bindings: []
custom_properties:
  - name: target
    type: string
    default: required
    description: Lowercase language id, package apiPrefix, or first-party package name such as rust or @clay/rust.
  - name: tier
    type: string
    default: required
    description: native, wasm, javascript, or js alias.
security: does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package-manager, native-library, WASM artifact, client-side JavaScript, or raw-op authority. It only records user-initiated engine selection for already-validated first-party syntax packages; packages cannot silently promote themselves over native tier without this preference.
agent_guidance: Prefer zero-config native defaults. Use only in init.js or explicit package-load setup when testing/forcing wasm or JavaScript parser fallback.
lookup_tags: [js-api, syntax, tree-sitter, engine, configuration, init, phase18.16]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# setSyntaxEnginePreference

## Summary

Forces Clay's syntax engine tier for a language or first-party package during `~/.config/clay/init.js` or package-load setup.

## Description

Records a startup/package-load preference used by the syntax grammar registry when selecting Tier 1 native, Tier 2 WASM, or Tier 3 JavaScript fallback.

## When to use

Use only when normal zero-config native syntax is not wanted, such as testing a WASM grammar artifact or forcing Markdown back to its package JavaScript parser.

## JavaScript usage

```ts
import { setSyntaxEnginePreference } from "clay:syntax";

setSyntaxEnginePreference("rust", "wasm");       // allow explicit Tier 2 override
setSyntaxEnginePreference("markdown", "javascript"); // use package JS parser fallback
```

## Example

```ts
import { setSyntaxEnginePreference } from "clay:syntax";

setSyntaxEnginePreference("typescript", "native");
```

## Defaults

No call is needed for normal use. With no preference, first-party languages use Tier 1 native Tree-sitter for Rust, TypeScript, TSX, JavaScript, and Markdown. Packages without a syntax grammar keep using their package JavaScript parse handler as Tier 3 fallback.

## Options

No options object is accepted; pass `target` and `tier` as positional string arguments.

## Parameters

- `target`: lowercase language id, package API prefix, or first-party package name such as `rust`, `markdown`, `@clay/rust`, or `@clay/markdown`.
- `tier`: `native`, `wasm`, or `javascript`; `js` is accepted as an alias for `javascript`.

## Key bindings

None by default. This API is intended for `init.js`, not direct key dispatch.

## Custom properties

- `target`
  - name: target
  - type: string
  - default: required
  - description: language id, package API prefix, or first-party package name.
- `tier`
  - name: tier
  - type: string
  - default: required
  - description: `native`, `wasm`, `javascript`, or `js` alias.

## Return and async behavior

Returns `undefined` after recording the preference. Synchronous facade; no await needed.

## Behavior

- `native`: prefer the compiled Tier 1 first-party grammar.
- `wasm`: allow an explicit Tier 2 `tree-sitter-wasm` override from an already-validated first-party package artifact.
- `javascript`: suppress syntax-grammar selection so registered package JS parse handlers remain the fallback.

Preferences are applied during startup, package load, document open, reload, or reclassification. Keypress, paint, layout, scroll, pointer, text-event, edit acknowledgement, parse-result publication, and decoration rendering paths never run configuration JavaScript or recompute preferences.

## Errors

Throws `clay.syntax.invalid_engine_preference` when `target` is empty/unsafe or `tier` is not `native`, `wasm`, `javascript`, or `js`.

## Permissions and security

This API records a user-initiated preference only. It does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package-manager, native-library, WASM artifact, client-side JavaScript, raw-op, third-party grammar, package enable/disable, raw CSS/color, native-widget, parser callback, or arbitrary artifact authority. It does not load packages, fetch grammar artifacts, or build WASM.

## Agent guidance

Prefer zero-config Tier 1 native defaults. Use this only when a user explicitly wants Tier 2 WASM or Tier 3 JavaScript fallback.

## Backing implementation

- Facade: `runtime/js/syntax.js::setSyntaxEnginePreference`
- Deno op: `src/server/ops/syntax.rs::op_clay_syntax_set_engine_preference`
- Rust owner: `src/server/syntax.rs::SyntaxGrammarRegistry::set_engine_preference`

## Lookup metadata

- id: `clay.syntax.setSyntaxEnginePreference`
- module: `clay:syntax`
- export: `setSyntaxEnginePreference`
- lookup tags: `js-api`, `syntax`, `tree-sitter`, `engine`, `configuration`, `init`, `phase18.16`
