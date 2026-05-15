# Clay JavaScript Facade Skeleton

`runtime/js/` contains the planned Clay JavaScript/TypeScript facade source tree. These files define the stable user-facing module layout that future server-side `deno_core` runtime work will bind to explicit Rust op wrappers.

## Module specifiers

Future import-map/runtime wiring should expose these files with Clay-owned module specifiers:

- `clay:editor` -> `runtime/js/editor.ts`
- `clay:keybindings` -> `runtime/js/keybindings.ts`
- `clay:configuration` -> `runtime/js/configuration.ts`
- `clay:documents` -> `runtime/js/documents.ts`
- `clay:behavior` -> `runtime/js/behavior.ts`

`runtime/js/mod.ts` is an aggregate source-tree entry point for organization and deterministic checks. User code should import domain modules rather than raw Rust functions or raw op names.

## Phase boundary

The facade exports are typed planned stubs. They intentionally throw a planned-runtime error and do not call `Deno.core.ops`, execute arbitrary JavaScript in the Rust client, or add work to Masonry paint/input handlers.

Phase 11 runtime work is expected to add explicit `#[deno_core::op2]` Rust wrappers registered through `deno_core::extension!`, then bind those wrappers behind these facades. Raw `op_*` names remain implementation details and must not become the public JavaScript API.

## Authority and security

This skeleton grants no filesystem, network, shell, extension loading, AI mutation, workspace, package, or client-side JavaScript execution authority. Authority-bearing behavior must be represented by documented Clay JS APIs, inventory records, and server-side validation before it becomes executable.
