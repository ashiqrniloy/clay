---
id: clay.application.quit
kind: clay-js-api
js_module: "clay:application"
js_export: quit
js_facade: runtime/js/application.ts::quit
backing_rust: src/masonry_editor.rs::EditorAction::ExitRequested
deno_op: op_clay_application_quit
deno_op_path: src/server/ops/application.rs::op_clay_application_quit
name: quit
user_facing_name: Quit Clay
summary: Quit Clay through the planned `clay:application` Clay JavaScript facade.
owner: client
phase: Phase 7
visibility: public
permissions: []
key_bindings: [Escape]
custom_properties: []
security: Requests application shutdown only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.
agent_guidance: Use `clay.application.quit` only for its documented application responsibility; prefer the Clay JS facade over raw Rust functions, protocol DTOs, or `Deno.core.ops` names.
lookup_tags: [application, escapequitapplicationactions, js-api]
app_visible: true
help_visible: true
stability: planned
async: false
---

# quit

## Summary

Quit Clay through the planned `clay:application` Clay JavaScript facade.

## Description

`quit` is the planned public API for **Quit Clay**. It is documented now so generated help, registry, configuration, and agent lookup work can target a stable Clay JS name instead of raw Rust symbols or future raw op wrappers.

Authority: `client-application-lifecycle`. Runtime path: `client-local-application-action`. Escape currently submits a local application action; it does not execute JavaScript or wait on IPC.

## When to use

Use this API when JavaScript configuration, extensions, or future Clay automation need the documented `Quit Clay` behavior. Do not use lower-level protocol structures, Rust functions, or raw `Deno.core.ops` bindings for this capability.

## JavaScript usage

```ts
import { quit } from "clay:application";

quit({ reason: "user-request" });
```

## Example

```ts
quit({ reason: "user-request" });
```

## Options

- `reason` (`string`): Optional future reason string for diagnostics or shutdown prompts.

## Key bindings

Default key bindings:

- `Escape`

Users may rebind or remove these through documented key binding APIs in `~/.config/clay/init.js`.

## Custom properties

No behavior-changing custom properties are defined for this API.

## Return and async behavior

Returns after requesting local application shutdown; no external authority is granted.

Current Phase 7 facade/runtime status is `planned`; this page defines the public contract before executable `deno_core` op wiring exists.

## Errors

The planned runtime should fail if arguments are malformed, the referenced document or editor surface does not exist, required permissions are absent, or server/client state rejects the requested operation. Current Phase 7 stubs throw a planned-runtime error rather than performing the operation.

## Permissions and security

No additional permission is required beyond access to the running editor session.

Requests application shutdown only; does not grant filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority.

Schema metadata records authority requirements only; it does not grant permissions, execute scripts, load extensions, inspect user files, access the network, or expose runtime user content.

## Agent guidance

Use `clay.application.quit` when the user asks for quit clay through the Clay JS API. Avoid inventing direct Rust calls, raw op names, filesystem effects, network effects, shell commands, AI mutation, workspace access, package loading, WASM, or client-side JavaScript execution for this operation.

## Backing implementation

- JS facade: `runtime/js/application.ts::quit`
- Future Deno op: `src/server/ops/application.rs::op_clay_application_quit` (`op_clay_application_quit`)
- Backing Rust/current owner: `src/masonry_editor.rs::EditorAction::ExitRequested`
- Current implementation audit path: `src/masonry_editor.rs::EditorWidget::on_text_event; src/main.rs::Driver::on_action`

## Lookup metadata

- Stable ID: `clay.application.quit`
- User-facing name: Quit Clay
- Kind: `clay-js-api`
- Module/export: `clay:application` / `quit`
- Default key bindings: `Escape`
- Custom properties: none
- Tags: `application`, `escapequitapplicationactions`, `js-api`
