# Clay JS Facade Skeleton

## Source

- `runtime/js/editor.ts`
- `runtime/js/keybindings.ts`
- `runtime/js/configuration.ts`
- `runtime/js/documents.ts`
- `runtime/js/behavior.ts`
- `runtime/js/application.ts`
- `runtime/js/mod.ts`
- `runtime/js/README.md`
- `docs/reference/clay-js-api/api-inventory.toml`
- `docs/reference/clay-js-api/inventory.md`
- `docs/reference/clay-js-api/*/*.md`
- `docs/index.md`
- `tests/clay_js_facade_layout.rs`
- `tests/clay_js_api_inventory.rs`

## Overview

The Clay JS facade skeleton defines the planned user-facing JavaScript/TypeScript source tree for future Clay runtime work. It gives each public domain a stable source file and typed planned exports while keeping raw Rust functions and future `deno_core` op wrappers out of the public API.

The Phase 7 inventory adds a machine-readable classification of current editor, protocol, behavior, key binding, configuration, document/lease, and application functionality. It records which capabilities are planned public Clay JS APIs, which runtime path they use, and which implementation details are intentionally internal.

The Phase 7 reference docs add one Markdown page for each public planned inventory API and link those pages from `docs/index.md` under **Clay JS API Registry Source Files**. Those Markdown pages are the public source of truth for generated app/help/agent registry work; this wiki page explains the implementation structure behind them instead of duplicating the full public API reference.

## Responsibilities

- Define stable domain module files for `clay:editor`, `clay:keybindings`, `clay:configuration`, `clay:documents`, `clay:behavior`, and `clay:application`.
- Provide typed planned stubs that document function shapes without performing runtime work.
- Preserve the boundary that raw `op_*` wrappers and Rust paths are implementation details behind Clay JS facades.
- Avoid loading configuration, executing arbitrary JavaScript in the Rust client, or adding work to editor input/paint hot paths.
- Classify public/planned APIs by stable ID, JS module/export, authority, runtime path, hot-path policy, backing Rust owner, future op name, docs path, key binding metadata, custom property metadata, permissions, and security notes.
- Record internal-only implementation details with `registry_public = false` so future registry generation excludes them deterministically.

## How It Works

Each facade file exports TypeScript option/result types and planned functions for its domain. The functions currently discard their arguments and call a local `plannedApi` helper that throws a clear planned-runtime error. This makes the source tree concrete for inventory, docs, and validation tasks without wiring `deno_core` or granting authority.

`runtime/js/mod.ts` re-exports the domain files as namespaces for source-tree organization and tests. Runtime import-map work can later map the individual files to Clay-owned module specifiers such as `clay:editor`, `clay:keybindings`, and `clay:application`.

`docs/reference/clay-js-api/api-inventory.toml` is the inventory source used by validation tests. Each `[[api]]` table has the same required metadata keys. Public/planned entries use the `clay.*` stable ID namespace, point to reference docs under `docs/reference/clay-js-api/`, and include negative security authority notes. Internal entries use `internal.*`, have no JS module/export, and set `registry_public = false`.

Per-API Markdown files use the schema in `docs/reference/clay-js-api/schema.md`: frontmatter captures stable IDs, modules/exports, facade paths, Rust/op mappings, permissions, key bindings, custom properties, lookup tags, visibility, and stability; body sections explain usage, examples, options, async behavior, errors, security, agent guidance, backing implementation, and lookup metadata. `docs/index.md` is the explicit registry inclusion list, so a public inventory entry is not registry-ready until its documentation path appears there.

## Code Examples

```ts
import { serverInsertText, clientSetCursorStyle } from "clay:editor";

await serverInsertText({ documentId: "current", offset: 0, text: "hello" });
clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
```

These calls are planned examples. In the checked-in skeleton they throw until future runtime and op wiring is implemented.

## Invariants and Constraints

- Facade exports must use concise lower-camel-case Clay JS names, not Rust or op names.
- Editor-core state APIs use `server*` or `client*` prefixes when authority matters.
- Facade files must not call raw `Deno.core.ops` functions.
- Inventory entries must classify hot-path client-first behavior separately from server-first/background work and explicitly preserve asynchronous ordinary typing.
- Internal implementation records in the inventory must not be treated as public registry source files.
- The skeleton grants no filesystem, network, shell, extension loading, AI mutation, workspace, package, or client-side JavaScript execution authority.
- The skeleton does not participate in Masonry paint/input handlers or the ordinary typing hot path.

## Tests

- `tests/clay_js_facade_layout.rs`: verifies expected domain files and planned exports exist, rejects raw op-shaped exports, rejects redundant names, and checks facade files do not call raw `Deno.core.ops`.
- `tests/clay_js_api_inventory.rs`: parses the inventory, checks required fields and duplicate IDs, verifies required Phase 7 functionality categories, confirms hot-path async notes, ensures internal-only records are not public registry APIs, validates per-API Markdown frontmatter/body sections, checks `docs/index.md` exactly matches public inventory docs, confirms docs match inventory IDs/modules/exports/facade paths/Rust/op metadata, verifies facade paths export the named functions, enforces Clay JS naming/authority-marker conventions, and validates security, key binding, and custom property metadata.
- Relevant commands: `cargo test clay_js_facade --test clay_js_facade_layout` and `cargo test --test clay_js_api_inventory`.
- Full verification used when adding the skeleton: `cargo fmt --check`, `cargo test`, and `cargo check`.

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- `docs/reference/clay-js-api/schema.md`
- `docs/reference/clay-js-api/configuration.md`
- `docs/reference/clay-js-api/inventory.md`
- `docs/reference/clay-js-api/editor/server-insert-text.md`
- `docs/index.md`
- `plans/008-Phase7-Clay-JS-API-Structure-and-Current-Functionality-Inventory.md`
