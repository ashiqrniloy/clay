# Clay JS Facade Skeleton

## Source

- `runtime/js/*.js` and `runtime/js/*.d.ts`
- `runtime/js/mod.ts`
- `runtime/js/README.md`
- `src/server/facades.rs`
- `src/server/js_runtime/mod.rs`
- `docs/reference/clay-js-api/api-inventory.toml`
- `docs/reference/clay-js-api/inventory.md`
- `docs/reference/clay-js-api/*/*.md`
- `docs/index.md`
- `tests/clay_js_facade_layout.rs`
- `tests/clay_js_api_inventory.rs`

## Overview

The Clay JS facade source tree defines the user-facing JavaScript/TypeScript API shape for Clay runtime work. Each domain has one checked-in executable `*.js` body and one adjacent declaration-only `*.d.ts` contract. `src/server/facades.rs` includes the JavaScript at compile time and assigns trusted-only/public runtime access; `src/server/js_runtime/mod.rs` contains no embedded facade body. Raw Rust functions and `deno_core` op wrappers stay outside the public API.

The Phase 7 inventory adds a machine-readable classification of current editor, protocol, behavior, key binding, configuration, document/lease, and application functionality. Phase 8 extends that contract with `clay:configuration` entry point APIs for `~/.config/clay/init.js`: `loadConfigurationModule` for local modular configuration and `getConfigurationState` for read-only configuration metadata. Phase 9 adds planned file/workspace facades for server-owned document open/save/reload/status/list behavior plus workspace-root metadata discovery: `serverOpenDocument`, `serverSaveDocument`, `serverReloadDocument`, `serverGetDocumentStatus`, `serverListDocuments`, and `serverListWorkspaceRoots`. It also verifies the initial editor customization contract for `clientSetCursorStyle` and keeps `clientSetViewport` classified as client-local viewport metadata rather than user configuration. Phase 12 adds `clay:sdui` schema helper stubs for defining inert panels, labels, buttons, lists, editor views, flex layouts, and stack layouts. Phase 13 runtime-backs `clay:configuration`, SDUI node helper/publication facades, the Phase 9 configuration-needed document/workspace subset, and keybinding/behavior manifest registration/query facades through explicit ops. Phase 16.5 adds controlled-runtime facade modules for `clay:packages`, `clay:modes`, and `clay:commands`: package manifest/permission validation, mode pattern registration/classification/major-mode activation, and package command registration/listing route through typed Rust primitive validators and registries. Phase 17 wires `packages.serverLoadPackage` through the full package record assembler and promotes it into the public generated Clay JS API registry, adds planned `serverSelectDocumentManifest`, and makes `clay:decorations`/`clay:parse` importable handoff modules. Phase 18 promotes the Markdown-required `decorations.serverPublishDecorations` and `parse.serverRegisterParseHandler` contracts to runtime-backed facades with explicit op wrappers and docs/registry entries. Phase 18.10 adds the runtime-backed `clay:syntax.serverRegisterSyntaxGrammar` facade/op for first-party grammar-only package load entries, documented in the generated Clay JS API registry. Phase 18.3 adds the runtime-backed `clay:ui` facade for inert package UI contribution declarations: `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`. It also preserves reviewed package/mode/configuration and package UI override surfaces as planned `clay:configuration` or `clay:ui` exports (`setPackageOption`, `setModePreference`, `setDecorationTheme`, `setParsePolicy`, `serverRegisterUiStateScope`, and `serverSetLayoutOverride`) so future settings remain documented APIs instead of ad hoc keys. Deferred editor/application APIs, older document lease/snapshot helpers, package enable/disable configuration, `serverSelectDocumentManifest`, direct working-area/split/slot layout APIs, and unpromoted provider hooks still resolve as facades that fail through a clear planned-unavailable op. The Phase 28 `clay:folding` publication facade is now runtime-backed; fold collapse and the five editor command-ID helpers remain covered by the trusted `clay:editor` facade.

The Phase 7 reference docs add one Markdown page for each public planned inventory API and link those pages from `docs/index.md` under **Clay JS API Registry Source Files**. Those Markdown pages are the public source of truth for generated app/help/agent registry work; this wiki page explains the implementation structure behind them instead of duplicating the full public API reference.

## Responsibilities

- Define stable domain module files for `clay:editor`, `clay:keybindings`, `clay:configuration`, `clay:documents`, `clay:workspace`, `clay:behavior`, `clay:application`, `clay:sdui`, and `clay:ui`, plus runtime module sources for primitive gates under `clay:packages`, `clay:modes`, `clay:commands`, `clay:decorations`, `clay:parse`, and `clay:syntax`.
- Keep `clay:configuration` focused on the server-side configuration entry point/module metadata contract; in Phase 13 it can load only explicit local relative `.js` modules below the configuration root and must not grant package/extension/workspace scan/network/shell/client filesystem authority.
- Keep package options, mode preferences, decoration theme preferences, and parse policy preferences planned until concrete validators and behavior-changing settings are implemented; package enable/disable remains out of scope.
- Keep editor customization APIs as planned facade metadata: `clientSetCursorStyle` exposes `color`, `blinking`, and `type` customization properties, while `clientSetViewport` remains client-local layout/paint metadata.
- Keep Phase 9 file/workspace APIs server-first: `clay:documents` owns document lifecycle and dirty-state metadata facades, while `clay:workspace` owns workspace-root metadata facades. The Phase 13 runtime-backed subset routes through server workspace validation and does not grant direct client filesystem authority.
- Keep Phase 12/13 SDUI helpers declarative and inert: `clay:sdui` helper calls construct node definition objects through server runtime ops, and `publishTree` validates/publishes the object graph through server SDUI state rather than client-side script execution, layout hot-path work, filesystem/network/shell access, or direct protocol DTO access.
- Keep Phase 18.3 package UI contribution APIs declarative and inert: `clay:ui` functions call Clay-owned ops that validate package manifests, prefixes, slots, component trees, action targets, theme tokens, payload budgets, and prohibited authority fields before storing records in internal Rust registries.
- Provide typed planned-unavailable facades for APIs whose runtime work is intentionally deferred.
- Preserve the boundary that raw `op_*` wrappers and Rust paths are implementation details behind Clay JS facades.
- Avoid loading configuration, executing arbitrary JavaScript in the Rust client, or adding work to editor input/paint hot paths.
- Classify public/planned APIs by stable ID, JS module/export, authority, runtime path, hot-path policy, backing Rust owner, future op name, docs path, key binding metadata, custom property metadata, permissions, and security notes.
- Record internal-only implementation details with `registry_public = false` so future registry generation excludes them deterministically.

## How It Works

Each executable facade exports functions for one domain; its adjacent declaration file carries TypeScript option/result types. `src/server/facades.rs` has one static 23-row table mapping `clay:*` specifiers to `include_str!("../../runtime/js/<domain>.js")` and a per-row access classification. `ClayModuleLoader` uses that table both to resolve source and to deny trusted-only modules in the third-party domain. This keeps facade loading compile-time/static with no runtime filesystem read, transpilation, allocation, or second body. Facades call Clay-owned ops internally; configuration/package code imports documented functions and does not expose raw `Deno.core.ops.op_*` names. `serverPublishDecorations`, `serverRegisterParseHandler`, and `serverPublishFoldingRanges` call explicit ops that validate package permissions/provenance and bounded provider metadata. Deferred APIs in editor and application modules, older document lease/snapshot helpers, package loading, package/mode configuration setters, `serverSelectDocumentManifest`, and provider hooks other than folding publication throw clear planned-unavailable errors through a shared op. In `runtime/js/documents.js`, Phase 9 document metadata mirrors the protocol's document id, version, access/lease, dirty flag, workspace root id, and workspace-relative path. `serverOpenDocument` and `serverReloadDocument` are explicit snapshot-returning server-first calls; `serverSaveDocument`, `serverGetDocumentStatus`, and `serverListDocuments` return metadata only. In `runtime/js/workspace.js`, `serverListWorkspaceRoots` exposes configured server root metadata without path expansion or file access. In `runtime/js/keybindings.js`, `bindKey` and `unbindKey` accept `global`/`editor` scopes while `listKeyBindings` accepts an additional `all` scope filter to match the documented configuration query default. In `runtime/js/sdui.js`, helper names mirror the Rust SDUI node kinds (`Panel`, `Label`, `Button`, `List`, `EditorView`, `Flex`, and `Stack`) as lower-camel-case `define*` exports backed by `op_clay_sdui_define_node`; `publishTree` calls `op_clay_sdui_publish_tree`, which converts the JSON object graph into a typed validated server `SduiTree`. In `runtime/js/ui.js`, `serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken` encode manifests/declarations to JSON, call the matching `op_clay_ui_*` wrapper, and parse structured registration summaries with provenance and payload metadata.

`runtime/js/mod.ts` re-exports domain declarations as namespaces for tooling. Runtime resolution does not use this aggregate; it uses the explicit `src/server/facades.rs` table.

`docs/reference/clay-js-api/api-inventory.toml` is the inventory source used by validation tests. Each `[[api]]` table has the same required metadata keys. Public/planned entries use the `clay.*` stable ID namespace, point to reference docs under `docs/reference/clay-js-api/`, and include negative security authority notes. Internal entries use `internal.*`, have no JS module/export, and set `registry_public = false`.

Per-API Markdown files use the schema in `docs/reference/clay-js-api/schema.md`: frontmatter captures stable IDs, modules/exports, facade paths, Rust/op mappings, permissions, key bindings, custom properties, lookup tags, visibility, and stability; body sections explain usage, examples, options, async behavior, errors, security, agent guidance, backing implementation, and lookup metadata. `docs/index.md` is the explicit registry inclusion list, so a public inventory entry is not registry-ready until its documentation path appears there.

## Code Examples

```ts
import { serverInsertText, clientSetCursorStyle } from "clay:editor";

await serverInsertText({ documentId: "current", offset: 0, text: "hello" });
clientSetCursorStyle({ color: "#ffcc00", blinking: true, type: "bar" });
```

Editor calls remain planned examples. The Phase 13 `clay:documents`/`clay:workspace` subset is executable only inside the server-side runtime with Clay ops installed.

## Invariants and Constraints

- Facade exports must use concise lower-camel-case Clay JS names, not Rust or op names.
- Editor-core state APIs use `server*` or `client*` prefixes when authority matters.
- User configuration must not call raw `Deno.core.ops` functions; facade implementations may call Clay-owned ops internally.
- File/workspace facade calls are server-first runtime APIs when Clay ops are installed and must stay out of ordinary keypress-to-paint, Masonry layout/paint, and edit acknowledgement hot paths.
- Inventory entries must classify hot-path client-first behavior separately from server-first/background work and explicitly preserve asynchronous ordinary typing.
- Internal implementation records in the inventory must not be treated as public registry source files.
- The skeleton grants no filesystem, network, shell, extension loading, AI mutation, workspace, package, document mutation from UI customization, native widget, raw CSS, raw op, renderer callback, or client-side JavaScript execution authority.
- The skeleton does not participate in Masonry paint/input handlers or the ordinary typing hot path.

## Tests

- `src/server/facades.rs` unit test: verifies 23 unique facade rows and exactly 14 public-third-party rows. Representative runtime-backed sources include `runtime/js/syntax.js`, `runtime/js/completion.js`, `runtime/js/folding.js`, and `runtime/js/packages.js`; each has an adjacent `.d.ts` declaration.
- `src/server/js_runtime/mod.rs` unit tests: verify curated imports execute directly from included files, route valid package/mode/command/load/parse/decoration/UI fixtures through Rust validators, reject invalid authority, and preserve planned-unavailable exports.
- `tests/clay_js_facade_layout.rs`: verifies all 21 executable files and declarations expose matching functions, every JavaScript file is included exactly once by the runtime table, no raw-string facade remains in `js_runtime/mod.rs`, and exports reject op-shaped/redundant names.
- `tests/clay_js_api_inventory.rs`: parses the inventory, checks required fields and duplicate IDs, verifies required Phase 7 functionality categories, confirms hot-path async notes, ensures internal-only records are not public registry APIs, validates per-API Markdown frontmatter/body sections, checks `docs/index.md` exactly matches public inventory docs, confirms docs match inventory IDs/modules/exports/facade paths/Rust/op metadata, verifies facade paths export the named functions, enforces Clay JS naming/authority-marker conventions, validates security/key binding/custom property metadata, checks package record loading is runtime-backed while per-document manifest selection and Phase 18 provider APIs remain planned, and records the Phase 17 configuration review: package/mode/decoration/parse configuration setters stay planned with custom-property metadata, package enable/disable is not configuration, and `clay:sdui.queryUiState` remains deferred/internal.
- `tests/clay_js_doc_registry.rs`: verifies the generated registry contains the package/mode/command primitive gate API docs, promoted `serverLoadPackage`, Phase 18 parse/decoration APIs, and Phase 18.3 `clay:ui` contribution APIs while still excluding planned-only state/layout override surfaces.
- `tests/rust_visibility_api_mapping.rs`: verifies primitive gate Rust validators/registries, op wrappers, and facade exports are represented in `api-inventory.toml`.
- Relevant commands: `cargo test clay_js_facade --test clay_js_facade_layout`, `cargo test --test protocol package_loading_docs::`, `cargo test --test protocol clay_js_api_inventory::`, `cargo test --test protocol clay_js_doc_registry::`, `cargo test --test protocol primitives_docs::`, and `cargo test --test security rust_visibility_api_mapping::`.
- Full verification used when adding the skeleton: `cargo fmt --check`, `cargo test`, and `cargo check`.

## Related

- [Behavior Manifests](behavior-manifests.md)
- [Client Behavior Routing](../flows/client-behavior-routing.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- `docs/reference/clay-js-api/schema.md`
- `docs/reference/clay-js-api/configuration.md`
- `docs/reference/clay-js-api/inventory.md`
- `docs/reference/clay-js-api/editor/server-insert-text.md`
- `docs/index.md`
- `plans/008-Phase7-Clay-JS-API-Structure-and-Current-Functionality-Inventory.md`
- `plans/009-Phase8-Configuration-Foundation.md`
