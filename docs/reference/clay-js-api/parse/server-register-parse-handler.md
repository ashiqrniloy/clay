---
id: clay.parse.serverRegisterParseHandler
kind: clay-js-api
js_module: "clay:parse"
js_export: serverRegisterParseHandler
js_facade: runtime/js/parse.ts::serverRegisterParseHandler
backing_rust: src/server/parse_coordinator.rs::ParseCoordinator::register_handler
deno_op: op_clay_parse_register_parse_handler
deno_op_path: src/server/ops/parse.rs::op_clay_parse_register_parse_handler
name: serverRegisterParseHandler
user_facing_name: Register Parse Handler
summary: Register a token-backed server-side background parse provider for a validated package mode.
owner: server
phase: Phase 18
visibility: public
permissions: ['parse-document']
key_bindings: []
custom_properties:
  - name: module
    type: object
    default: required-for-runtime-bridge
    description: Package module object whose exported parser function stays inside the persistent server runtime behind a server-issued token.
  - name: exportName
    type: string
    default: default
    description: Export name resolved from module; the export must be a function and is never serialized to Rust.
  - name: modeId
    type: string
    default: required
    description: Active mode ID this handler serves.
  - name: parseUnit
    type: enum
    default: line-group
    description: Coarsest incremental parse unit: file, region, or line-group.
  - name: viewportPriority
    type: boolean
    default: true
    description: Whether visible ranges are prioritized before adjacent/off-viewport ranges.
  - name: timeoutMs
    type: number
    default: 50
    description: Bounded handler timeout policy validated at registration/configuration time.
  - name: maxWindowBytes
    type: number
    default: 65536
    description: Maximum bytes included in one bounded parse-window snapshot.
  - name: guardBytes
    type: number
    default: 4096
    description: Generic context bytes added around requested ranges while staying within the window cap.
  - name: memoryBudgetBytes
    type: number
    default: 31457280
    description: Retained syntax/window memory budget capped by SYNTAX_CACHE_BUDGET_BYTES.
  - name: resultBudgetBytes
    type: number
    default: 4096
    description: Incremental parse result budget enforced before publication.
security: Requires parse-document permission and server validation of package provenance, mode, parse unit, timeout, cancellation/stale-version behavior, bounded parse-window snapshots, syntax memory budget, and bounded parse result publication; handler functions stay in the persistent server runtime behind a server-issued token and executable handler/callback/onParse/function payload keys are rejected; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, client-side JavaScript, raw Deno ops, or access beyond Clay-provided open document content.
agent_guidance: Use `clay.parse.serverRegisterParseHandler` to declare a package-owned background parser with `{ module, exportName }`. Do not pass executable callbacks in registration payloads or put parse work on the client hot path. Treat `clay.runtime.timeout` as the diagnostic for a handler/runtime evaluation that exceeds its validated timeout budget.
lookup_tags: [js-api, parse, markdown, incrementalparseupdate, parser]
app_visible: true
help_visible: true
stability: runtime-backed
async: false
---

# serverRegisterParseHandler

## Summary

Registers a token-backed server-side package parser. The facade keeps the JavaScript function inside the persistent server runtime, Rust stores only a server-issued token plus validated metadata, and `ParseCoordinator` schedules actual parse work as cancellable background work after document edits or viewport changes.

## Description

`serverRegisterParseHandler` is the public Clay JS API for the Phase 18 `IncrementalParseUpdate` primitive. It finalizes the package-facing registration contract for Markdown-required parsing without exposing raw `Deno.core.ops` or allowing package JavaScript in Rust client hot paths.

As of Phase 18.7 the public API is runtime-backed: a resolver-validated package load entry may pass `{ module, exportName }`, and the facade records the selected export in `globalThis.__clayParseHandlers[token]` after op-side validation returns the token. `ParseWindowSnapshot`, `ParseWindowRequest`, parse-window scheduling, cancellation state, and syntax-cache accounting remain internal Rust/protocol primitives; packages receive only Clay-supplied bounded window payloads through the approved server runtime path.

## When to use

Use this API during package load/activation when a mode package can parse an open document and produce inert parse/decorations data. Markdown uses this to register line-group/region parsing for headings, emphasis, inline code, fenced code blocks, and list markers. The API field is `parseUnit`; older planning docs used the plural label `parseUnits` for the same behavior-changing parse policy choice.

## JavaScript usage

```ts
import { serverRegisterParseHandler } from "clay:parse";

import * as parserModule from "./parser.js";

serverRegisterParseHandler({
  packageManifest,
  mode: "markdown",
  parseUnit: "line-group",
  viewportPriority: true,
  module: parserModule,
  exportName: "parseMarkdownDecorationUpdate",
  maxWindowBytes: 64 * 1024,
  guardBytes: 4 * 1024,
  memoryBudgetBytes: 30 * 1024 * 1024,
  timeoutMs: 50,
});
```

## Example

```ts
const parserModule = await import("./parser.js");

serverRegisterParseHandler({
  packageManifest,
  mode: "markdown",
  parseUnit: "line-group",
  viewportPriority: true,
  module: parserModule,
  exportName: "parseMarkdownDecorationUpdate",
  maxWindowBytes: 64 * 1024,
  guardBytes: 4 * 1024,
  memoryBudgetBytes: 30 * 1024 * 1024,
  timeoutMs: 50,
});
```

## Options

- `packageManifest` or package context fields: package identity and declared `parse-document` permission.
- `module` (`Record<string, unknown>`, required for live handlers): Package module object already loaded inside the persistent server runtime.
- `exportName` (`string`, default `"default"`): Function export to store behind the server-issued token.
- `modeId` / `mode` (`string`, required): Mode ID served by the handler.
- `parseUnit` (`"file" | "region" | "line-group"`, default `"line-group"`): Incremental unit hint.
- `viewportPriority` (`boolean`, default `true`): Prioritize visible parse output.
- `timeoutMs` (`number`, default `50`): Bounded timeout policy; values must be between 1 and 5000.
- `maxWindowBytes` / `parseWindowBytes` (`number`, default `65536`): Maximum bytes Clay may include in one bounded parse-window snapshot.
- `guardBytes` (`number`, default `4096`): Generic context bytes Clay may add around requested viewport/invalidated ranges, still capped by `maxWindowBytes`.
- `memoryBudgetBytes` (`number`, default `31457280`): Retained syntax/window budget; values must be non-zero and at or below `SYNTAX_CACHE_BUDGET_BYTES`.
- `resultBudgetBytes` (`number`, default `4096`): Parse-result payload budget; runtime uses `INCREMENTAL_PARSE_UPDATE_BUDGET_BYTES`.

Registration payloads are token-backed, not callback-serialized. Executable fields such as `handler`, `callback`, `onParse`, or `function` are rejected by the public contract in both the TypeScript facade and Rust op. When `module` is supplied, the facade validates `module[exportName]` is a function and stores it in the persistent runtime registry by token; Rust never receives the JavaScript function value.

Large-file parse policy values are validated once at package load/registration or explicit configuration-promotion time, not per keypress or paint. Unsafe timeout, window, guard, or memory values are rejected before parser scheduling.

## Key bindings

No default key binding is assigned.

## Custom properties

- `modeId`: parser/mode binding.
- `module`: persistent-runtime parser module, stored behind a token.
- `exportName`: function export selected from `module`.
- `parseUnit`: incremental scheduling contract.
- `viewportPriority`: visible-range priority policy.
- `timeoutMs`: timeout/budget policy.
- `maxWindowBytes` / `parseWindowBytes`: bounded parse-window size policy.
- `guardBytes`: bounded parse-window guard context policy.
- `memoryBudgetBytes`: retained syntax/window memory policy.
- `resultBudgetBytes`: parse-result payload budget metadata.

## Return and async behavior

Returns JSON-serializable registration metadata synchronously from the server runtime, including the internal handler token. Scheduling and parse result delivery happen later as background work and must never block ordinary `ClientFirstPredictable` typing or local paint.

## Errors

Fails with Clay error codes when permissions are missing, package identity is malformed, mode is empty, parse unit is unsupported, timeout/window/memory budgets are out of bounds, executable callback fields are supplied, `module[exportName]` is not a function, or handler execution exceeds the smaller of the registered `timeoutMs` and the service runtime guard. Timeout failures surface through the `clay.runtime.timeout` diagnostic.

## Permissions and security

Requires: `parse-document`.

The API does not grant filesystem, network, shell, AI mutation, workspace mutation, package installation/enabling, WASM, raw ops, or client-side JavaScript authority. Parser input is limited to Clay-provided open document content and bounded edit/viewport/window metadata; large-file parse windows expose only validated byte ranges from already-open documents.

## Agent guidance

Prefer this facade over raw ops or direct Rust calls. Keep parse work cancellable, viewport-prioritized, and separate from document mutation. Do not expose internal parse-window snapshot structs or scheduler methods as package APIs unless a later phase promotes them with their own Clay JS facade, docs, registry entry, and validators. Use `clay.decorations.serverPublishDecorations` for validated decoration publication.

## Backing implementation

- JS facade: `runtime/js/parse.ts::serverRegisterParseHandler`
- Runtime facade: `src/server/js_runtime.rs::CLAY_FACADE_PARSE`
- Op wrapper: `src/server/ops/parse.rs::op_clay_parse_register_parse_handler`
- Coordinator: `src/server/parse_coordinator.rs::ParseCoordinator`
- Protocol type: `src/protocol/parse.rs::IncrementalParseUpdate`

## Lookup metadata

- Stable ID: `clay.parse.serverRegisterParseHandler`
- User-facing name: Register Parse Handler
- Lookup tags: `js-api`, `parse`, `markdown`, `incrementalparseupdate`, `parser`
- App/help visible: true
