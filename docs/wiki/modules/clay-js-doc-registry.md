# Clay JS Documentation Registry

## Source

- `src/docs/registry.rs`
- `src/bin/update-doc-registry.rs`
- `build.rs`
- `windows/no-uac.manifest`
- `docs/index.md`
- `docs/generated/clay-js-api-registry.json`
- `docs/reference/documentation-contracts.json`
- `tests/{clay_js_doc_registry,clay_js_api_inventory,primitives_docs,package_loading_docs}.rs`

## Overview

The Clay JS documentation registry turns the Markdown files linked under `docs/index.md`'s **Clay JS API Registry Source Files** section into a checked-in generated JSON artifact for future app/help/agent discovery. Markdown remains the source of truth; the generated artifact is derived output.

## Responsibilities

- Parse Clay JS API Markdown frontmatter from the explicit master-index link list.
- Validate required registry metadata, including stable IDs, JS facade mapping, future op mapping, key bindings, custom properties, lookup tags, and no-authority security notes.
- Statically enforce no-authority-by-default security language for configuration-relevant APIs in source docs, inventory metadata, and generated registry entries.
- Include Phase 8 configuration entry point APIs (`configuration.loadConfigurationModule` and `configuration.getConfigurationState`) so app/help/agent lookup can discover `~/.config/clay/init.js` and local modular configuration semantics without executing user code.
- Include Phase 9 file/workspace APIs (`documents.serverOpenDocument`, `serverSaveDocument`, `serverReloadDocument`, `serverGetDocumentStatus`, `serverListDocuments`, and `workspace.serverListWorkspaceRoots`) so app/help/agent lookup can discover server-owned file IO and workspace metadata capabilities without exposing raw protocol messages or filesystem authority.
- Verify key binding configuration APIs (`keybindings.bindKey`, `keybindings.unbindKey`, and `keybindings.listKeyBindings`) as planned server-side configuration/query APIs with empty default key-binding lists, queryable `key`/`command`/`scope`/`when` custom properties, command ID validation notes, and no external authority.
- Verify initial editor customization metadata for `editor.clientSetCursorStyle`, including generated `color`, `blinking`, and `type` custom properties with types, defaults, allowed values where relevant, lookup coverage, and no document-mutation or external authority.
- Include Phase 13 SDUI schema helper/publication APIs under `clay:sdui` (`definePanel`, `defineLabel`, `defineButton`, `defineList`, `defineEditorView`, `defineFlex`, `defineStack`, and `publishTree`) so app/help/agent lookup can discover runtime-backed inert server-driven UI construction without exposing raw protocol DTOs, native observability internals, or client-side script authority.
- Include Phase 18.3 package UI contribution APIs under `clay:ui` (`serverRegisterPanelContribution`, `serverRegisterComponentContribution`, `serverRegisterTransientOverlayContribution`, and `serverRegisterThemeToken`) so app/help/agent lookup can discover runtime-backed inert fixed panel, component, transient overlay, and theme-token declarations without exposing raw ops, Masonry widgets, native handles, raw CSS, or client-side JavaScript authority.
- Include Phase 18.4 public programmatic APIs (`ui.serverRegisterInputContribution`, `ui.serverRegisterUiStateScope`, `ui.serverSetLayoutOverride`, and `configuration.setPackageOption`) with Markdown docs, docs-index links, inventory rows, generated registry entries, custom-property lookup, app/help visibility, facade/op/Rust metadata, and security notes while keeping working-area, pane-split, and direct pane-slot mutation APIs planned and absent from the generated registry.
- Keep Phase 15 SDUI observability helpers (`SduiObservableSnapshot`, `SduiStatusObservation`, and their extraction methods) crate-internal unless a future dedicated Clay JS API adds docs, facade, op, inventory, and generated-registry metadata.
- Produce deterministic JSON ordered by stable API ID.
- Load the checked-in generated JSON with `ClayJsApiRegistry::from_generated` for app/help/agent discovery without reading source Markdown during normal lookup.
- Provide read-only lookup helpers for stable ID, JS module/export, user-facing name, kind/owner, lookup tag, default key binding, and custom property name.
- Provide a non-mutating stale check for tests and a developer update command for intentional artifact rewrites.
- Validate all API entries generically against `api-inventory.toml`, generated registry metadata, index links, facade exports, source paths, required sections, permissions, and authority fields. Primitive/package document coverage and the narrow exact security-marker set live in `documentation-contracts.json`; ordinary prose is not a test input.
- Enforce the Plan 060 public-surface audit: 105 classified inventory rows map to 86 generated public entries, while trust-domain identity, package context, output routing, close lifecycle, queue/file/scheduler budgets, and file identity remain private or `pub(crate)` and absent from every executable/declaration facade.

The registry code does not execute JavaScript, load `~/.config/clay/init.js`, call Deno ops, grant permissions, or run on editor paint/input hot paths.

## How It Works

1. `registry_source_paths` reads `docs/index.md`, extracts only links between **Clay JS API Registry Source Files** and **Registry Rules**, and normalizes them to `docs/...` paths.
2. `ClayJsApiRegistry::from_docs` parses each linked Markdown file's YAML frontmatter with a focused std-only parser. It accepts LF and CRLF opening frontmatter delimiters, and handles scalar fields, inline string lists, booleans, and the `custom_properties` object list used by the Clay JS API schema.
3. Validation rejects duplicate IDs, wrong kinds, empty lookup tags for public entries, and missing no-authority language for filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, or client-side JavaScript authority. Inventory/doc tests apply the same denied-authority list to configuration entry points, key binding APIs, and APIs with behavior-changing custom properties, and they require permission-bearing configuration-relevant APIs to document permission validation before entering the public registry.
4. `to_generated_json` serializes entries in stable ID order with deterministic field ordering.
5. `ClayJsApiRegistry::from_generated` parses the checked-in generated JSON embedded with `include_str!`, validates schema version `1`, validates entries with the same metadata/security rules, and returns typed `RegistryEntry` values.
6. Lookup helpers scan the typed entry list and return `Option<&RegistryEntry>` for unique keys (`by_id`, `by_js_export`) or `Vec<&RegistryEntry>` for multi-match queries (`by_user_facing_name`, `by_kind_owner`, `by_lookup_tag`, `by_key_binding`, `by_custom_property`). Empty vectors are the normal no-match state for list queries.
7. `check_generated_registry_current` compares expected generated bytes with `docs/generated/clay-js-api-registry.json` without writing files. On drift, tests print `cargo run --bin update-doc-registry`.
8. `src/bin/update-doc-registry.rs` is the explicit mutating developer command that rewrites the checked-in artifact. On Windows MSVC, `build.rs` embeds `windows/no-uac.manifest` into that binary so Windows does not classify the `update-*` executable name as requiring elevation during `cargo test --all-targets`.
9. `tests/rust_visibility_api_mapping.rs` independently scans defining Rust files and all `runtime/js/*.{js,d.ts}` facades. Bare-public internal declarations or leaked names fail the security suite. Plan 060 reduced newly added coordinator teardown/subscription methods, client close enqueueing, and compiled queue/filesystem/scheduler ceilings to `pub(crate)`; no new JS API was needed for those mechanics.

## Code Examples

```bash
cargo run --bin update-doc-registry
cargo test --test protocol clay_js_doc_registry::
```

```rust
let root = clay::docs::registry::repository_root();
clay::docs::registry::check_generated_registry_current(&root)?;

let registry = clay::docs::registry::ClayJsApiRegistry::from_generated()?;
let cursor_style = registry.by_id("editor.clientSetCursorStyle");
let configurable_color_apis = registry.by_custom_property("color");
```

## Invariants and Constraints

- `docs/index.md` plus Clay JS API Markdown is authoritative; do not hand-author registry entries as source data.
- `cargo test` must not mutate `docs/generated/clay-js-api-registry.json`.
- Windows test builds must run the `update-doc-registry` test binary as the current user; the embedded as-invoker manifest is build metadata only and does not grant elevation or additional OS authority.
- Registry generation and stale checks are developer/test operations only and add no runtime work to Masonry rendering, input handling, IPC frame handling, or ordinary edits.
- Public Rust items under `src/docs` are classified as internal documentation-registry infrastructure unless a future change promotes them through Clay JS API docs, inventory metadata, and generated registry coverage.
- Normal lookup uses generated/static JSON data and does not read source Markdown or regenerate artifacts.
- Lookup is read-only: it exposes documentation metadata only and never executes configuration files, JavaScript, or Deno ops.
- Configuration entry point entries are contract metadata only. `loadConfigurationModule` describes future server-side local module loading from `~/.config/clay/init.js`; Phase 8 does not read those files, evaluate JavaScript, load packages/extensions, access the network/workspace, run shell commands, or grant client-side JavaScript authority.
- File/workspace entries are contract metadata only until future `deno_core` op wrappers exist. The docs record required server-side validation, workspace root authorization, path traversal rejection, typed file errors, and no raw host filesystem authority; the generated registry does not perform IO or broaden workspace permissions.
- Editor customization entries are metadata contracts only. Cursor style remains configuration/customization UI state, while viewport sizing remains a planned client-local layout API rather than user configuration; neither path grants document mutation or routes ordinary typing through JavaScript.
- SDUI helper entries describe inert native UI nodes, action intents, and explicit validated publication; they do not expose native observability snapshots/status structs, execute client scripts, grant document/file/workspace authority, or run in Masonry paint/input hot paths.
- `clay:ui` contribution entries describe inert package UI declarations and explicit server validation; they do not expose raw `op_clay_ui_*` calls, native widgets, Masonry handles, raw CSS/style strings, raw colors outside typed token contracts, renderer callbacks, external authorities, or client-side JavaScript hooks. Phase 18.4 layout overrides are public only through the documented `serverSetLayoutOverride` typed override API and remain configuration/package-update work, not direct pane/working-area mutation.
- Security metadata records authority boundaries only; it does not grant permissions or execute configuration.
- Configuration-relevant APIs must deny implicit filesystem, network, shell, extension loading, AI mutation, workspace, package loading, WASM, and client-side JavaScript execution authority in both source documentation and generated registry metadata.
- `clay:packages`, `clay:documents`, and `clay:workspace` are trusted-only facades. Third-party packages cannot self-adopt, promote runtime domains, invoke trusted file/package controls, or obtain raw connection/document lifecycle handles.
- `serverSaveDocument` honors an explicit `knownVersion` upper bound, uses only an already-open document, and reaches disk through bounded atomic-save identity revalidation. Omission retains trusted configuration's server-internal baseline; this authority is unavailable in third-party runtime.
- `serverListDirectory` exposes bounded page/depth/cancellation inputs, not internal worker counts or ignore budgets; unsupported/oversized root-ignore syntax returns a bounded diagnostic.
- `loadPackage` remains the only implemented package activation API. Third-party adoption/revocation stays out-of-band host/CLI authority so package JavaScript cannot approve itself.

## Tests

- `tests/clay_js_doc_registry.rs::generated_registry_is_current`: stale-artifact check and repair command.
- `tests/clay_js_doc_registry.rs::generated_registry_contains_all_indexed_public_apis`: verifies master-index coverage and unique stable IDs.
- `tests/clay_js_doc_registry.rs::generated_registry_preserves_configuration_metadata`: verifies key binding, custom property, permission, security, facade, op, Rust owner, and lookup-tag metadata survives generation.
- `tests/clay_js_doc_registry.rs::generated_registry_contains_phase13_sdui_runtime_apis`: verifies Phase 13 SDUI helper/publication docs are generated under `clay:sdui`, keep empty default key bindings, preserve runtime-backed sync/async metadata, deny external authority, and are discoverable by SDUI lookup tags/custom properties.
- `tests/clay_js_doc_registry.rs::planned_shell_layout_apis_are_not_generated_registry_entries`: verifies planned shell/layout/state override APIs stay out of the public generated registry while implemented package UI contribution docs are generated under `clay:ui` with UI documentation paths.
- `tests/clay_js_doc_registry.rs::generated_registry_contains_phase18_4_public_apis`: verifies Phase 18.4 input, UI state-scope, layout override, and package-option APIs are generated, app/help visible, lookup-tagged, custom-property queryable, and keep no-authority security metadata.
- `tests/clay_js_doc_registry.rs::lookup_finds_api_by_stable_id_and_export`: verifies ID, JS module/export, user-facing name, kind/owner, and tag lookups over generated data.
- `tests/clay_js_doc_registry.rs::lookup_finds_configuration_by_custom_property`: verifies custom property discovery for cursor style configuration metadata.
- `tests/clay_js_doc_registry.rs::cursor_style_custom_properties_are_complete`: verifies `color`, `blinking`, and `type` include type/default metadata and that the enum documents `block`, `bar`, and `underline`.
- `tests/clay_js_doc_registry.rs::editor_customization_has_no_external_authority`: verifies cursor-style and viewport customization metadata deny document mutation and external authority.
- `tests/clay_js_doc_registry.rs::configuration_lookup_finds_cursor_customization`: verifies cursor customization is discoverable by lookup tag and by custom property name.
- `tests/clay_js_doc_registry.rs::lookup_lists_empty_default_key_bindings`: verifies empty default key-binding lists remain queryable and key-binding lookup finds defaults.
- `tests/clay_js_doc_registry.rs::keybinding_configuration_apis_have_empty_defaults`: verifies the three key binding management APIs are generated under `clay:keybindings`, expose empty default key-binding lists, retain keybinding lookup tags, and deny client-side JavaScript authority.
- `tests/clay_js_doc_registry.rs::keybinding_configuration_custom_properties_are_queryable`: verifies `bindKey` exposes exactly `key`, `command`, `scope`, and `when`, and that generated custom-property lookup finds key binding APIs by those properties.
- `tests/clay_js_doc_registry.rs::keybinding_docs_reject_undocumented_authority`: statically checks the key binding docs describe behavior-manifest routing, deny filesystem/network/shell/extension/AI/workspace/package/WASM/client-JS authority, and require documented/registered command IDs before binding.
- `tests/clay_js_doc_registry.rs::generated_registry_contains_phase9_file_workspace_apis`: verifies Phase 9 file/workspace APIs are generated, lookup-tagged, and preserve path-validation/no-filesystem-authority security notes.
- `tests/clay_js_doc_registry.rs::configuration_entrypoint_is_documented_and_indexed`: verifies `~/.config/clay/init.js` is documented and configuration facade entries are generated.
- `tests/clay_js_doc_registry.rs::configuration_module_loading_is_planned_no_authority`: verifies modular loading remains planned, local, key-binding-free metadata with no implicit external authority.
- `tests/clay_js_doc_registry.rs::lookup_is_read_only`: verifies lookup remains metadata-only and does not model local configuration files as executable entries.
- `tests/clay_js_doc_registry.rs::generated_registry_configuration_security_denies_implicit_external_authority`: verifies generated configuration-relevant metadata denies filesystem, network, shell, extension loading, AI mutation, workspace, package, WASM, and client-side JavaScript authority.
- `tests/clay_js_doc_registry.rs::generated_registry_security_matches_source_docs`: verifies generated registry security fields preserve source Markdown frontmatter exactly.
- `tests/rust_visibility_api_mapping.rs::third_party_facade_allowlist_exactly_matches_plan_public_inventory`: verifies the 13-facade third-party allowlist remains exact.
- `tests/rust_visibility_api_mapping.rs::internal_runtime_mechanics_are_not_public`: rejects bare-public trust-domain, package-context, routing, lifecycle, queue, filesystem, and scheduler declarations.
- `tests/rust_visibility_api_mapping.rs::internal_runtime_names_are_absent_from_public_facades`: rejects internal names and a public close-document callable in all JS/declaration facades.
- `tests/clay_js_api_inventory.rs`: nine generic validators enumerate every `[[api]]` row and fail with stable ID/field/path context for incomplete schemas, inventory/index/generated-matrix drift, Markdown metadata drift, missing sections/facades/source paths, naming/raw-op violations, permission/authority metadata, mutation, and unsafe paths.
- `tests/primitives_docs.rs`: `documentation_contract_inventory_is_complete_and_indexed` enumerates every primitive/package reference page; the primitive matrix validator checks every row/field; wiki indexing is recursive; only security markers listed in `documentation-contracts.json` remain phrase-sensitive.
- `tests/package_loading_docs.rs`: generic package-document rows bind language-package docs to package manifests, load entries, API inventory ownership, and the no-hidden-package-configuration rule.
- Synthetic validator checks prove missing fields/markers report exact IDs/paths and harmless prose rewrites pass. Validators read files only; only `cargo run --bin update-doc-registry` writes generated output.

## Phase 22.7: Split-Alias Entries

Phase 22.7 added `shell.clientSplitPaneRight` and
`shell.clientSplitPaneDown` as registry-public entries (123 total,
all public) with `key_bindings: []` (aliases — the canonical
`Ctrl+\`/`Ctrl+-` defaults stay on the vertical/horizontal entries) and
`phase: Phase 22.7`. They ride the same validation surface as every other
entry: facade exports in `runtime/js/shell.js` (+ `.d.ts`), master-index
links, inventory rows, and the generic validators — no new test code
needed; the drift guard (`public_inventory_docs_index_and_generated_matrix_match_exactly`)
fails until `cargo run --bin update-doc-registry` is re-run.

## Phase 22.8: Per-tab API boundary audit

Phase 22.8 adds server-owned tab routing, not a new JavaScript authority
surface. The changed server functions are intentionally internal:
`IpcServer::{create_tab_state, ensure_tab_state, tab_state,
tab_state_for_client, unbound_bootstrap_state, state_for_client,
remove_tab_state}`, `TabServerState::{workspace_pane_visible,
toggle_workspace_pane}`, `TabRegistry` lookup/mutation helpers, and
`WorkspaceState::with_document_id_allocator` are `pub(crate)`; connection
routing (`route_connection_tab_state`, `document_for_message`, and workspace
command result handling) remains private. The client reconnect/restore helpers
are native client plumbing, not server-side JavaScript APIs.

Existing documented APIs remain the public boundary: `shell.clientTabNew`
starts a picker-backed connection-bound `TabCommand::New`,
`documents.serverOpenDocument` and `clay.workspace` APIs operate through
server-owned workspace validation, and `commands.serverOpenFile`/
`serverOpenDirectory` reuse the command boundary. None accepts an arbitrary
`TabId` or exposes a `TabServerState` handle. `workspace.toggleFileBrowser`
is a fixed built-in command ID routed through the existing
`keybindings.bindKey` API, so it is documented in the keybinding/configuration
reference but is deliberately not a second callable facade or registry entry.
This keeps the per-tab workspace pane flag server-authoritative without adding
hidden configuration keys or implicit filesystem authority.

The audit is enforced by
`tests/rust_visibility_api_mapping.rs::phase22_8_per_tab_state_has_no_new_public_programmatic_surface`
and
`tests/clay_js_doc_registry.rs::phase22_8_programmatic_surface_inventory_is_closed`.
The first pins Rust visibility and rejects internal facade names; the second
pins existing facade/op/docs/lookup metadata, rejects arbitrary-tab IDs, and
requires the fixed file-browser command to remain documented through
`bindKey`. The authoritative usage pages are
[`clientTabNew`](../../reference/clay-js-api/shell/client-tab-new.md),
[`serverOpenDocument`](../../reference/clay-js-api/documents/server-open-document.md),
and [`serverListWorkspaceRoots`](../../reference/clay-js-api/workspace/server-list-workspace-roots.md).

## Related

- [Clay JS Facade Skeleton](clay-js-facade-skeleton.md)
- [Slot-Aware Package UI](slot-aware-package-ui.md)
- `docs/reference/clay-js-api/schema.md`
- `docs/reference/clay-js-api/inventory.md`
- `plans/009-Phase8-Configuration-Foundation.md`
